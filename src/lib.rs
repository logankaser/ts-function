use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, Ident, Item, ItemImpl, ItemType, ReturnType, Type, parse_macro_input};

struct ParsedSignature<'a> {
    struct_ident: &'a Ident,
    args: Vec<(Ident, &'a Type)>,
    output: &'a ReturnType,
}

#[proc_macro_attribute]
pub fn ts_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);

    let result = match &item {
        Item::Type(item_type) => parse_item_type(item_type),
        Item::Impl(item_impl) => parse_item_impl(item_impl),
        _ => {
            return Error::new_spanned(
                item,
                "#[ts_function] can only be applied to a type alias or an impl block",
            )
            .to_compile_error()
            .into();
        }
    };

    match result {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn parse_item_type(item_type: &ItemType) -> syn::Result<proc_macro2::TokenStream> {
    let Type::BareFn(bare_fn) = &*item_type.ty else {
        return Err(Error::new_spanned(
            &item_type.ty,
            "Expected a function pointer type (e.g., `fn(x: f64)`)",
        ));
    };

    let struct_ident = &item_type.ident;
    let mut args = Vec::new();

    for (i, arg) in bare_fn.inputs.iter().enumerate() {
        let ident = match &arg.name {
            Some((ident, _)) => ident.clone(),
            None => format_ident!("arg{}", i),
        };
        args.push((ident, &arg.ty));
    }

    let parsed = ParsedSignature {
        struct_ident,
        args: args.clone(),
        output: &bare_fn.output,
    };

    let abi_traits = generate_abi_traits(&parsed)?;

    let mut fn_args = Vec::new();
    let mut arg_conversions = Vec::new();
    let mut call_args = Vec::new();
    for (ident, ty) in &args {
        fn_args.push(quote! { #ident: #ty });

        let conversion = if let Type::ImplTrait(type_impl) = *ty {
            // Find `Into<X>`
            let mut inner_ty_tokens = None;
            for bound in &type_impl.bounds {
                if let syn::TypeParamBound::Trait(trait_bound) = bound
                    && let Some(segment) = trait_bound.path.segments.last()
                    && segment.ident == "Into"
                    && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    inner_ty_tokens = Some(quote! { #inner_ty });
                    break;
                }
            }
            if let Some(inner) = inner_ty_tokens {
                quote! {
                    let #ident = ::std::convert::Into::<#inner>::into(#ident);
                    let #ident = ::std::convert::Into::<::wasm_bindgen::JsValue>::into(#ident);
                }
            } else {
                return Err(Error::new_spanned(
                    ty,
                    "Unsupported `impl Trait`. Only `impl Into<T>` is supported.",
                ));
            }
        } else {
            quote! {
                let #ident = ::std::convert::Into::<::wasm_bindgen::JsValue>::into(#ident);
            }
        };

        arg_conversions.push(conversion);
        call_args.push(quote! { &#ident });
    }

    let call_method = match call_args.len() {
        0 => quote! { call0(&::wasm_bindgen::JsValue::NULL) },
        1 => quote! { call1(&::wasm_bindgen::JsValue::NULL, #(#call_args),*) },
        2 => quote! { call2(&::wasm_bindgen::JsValue::NULL, #(#call_args),*) },
        3 => quote! { call3(&::wasm_bindgen::JsValue::NULL, #(#call_args),*) },
        _ => {
            return Err(Error::new_spanned(
                item_type,
                "Functions with more than 3 arguments are not supported yet",
            ));
        }
    };

    let output = parsed.output;
    let ret_stmt = if matches!(output, ReturnType::Default) {
        if cfg!(feature = "console") {
            quote! { 
                if let Err(e) = self.0.#call_method {
                    ::web_sys::console::error_1(&e);
                }
            }
        } else {
            quote! { 
                if let Err(e) = self.0.#call_method {
                    panic!("JavaScript exception: {:?}", e);
                }
            }
        }
    } else {
        return Err(Error::new_spanned(
            output,
            "Return types are not supported in the type alias pattern. Use the `impl` escape hatch instead.",
        ));
    };

    Ok(quote! {
        pub struct #struct_ident(pub ::js_sys::Function);

        impl #struct_ident {
            pub fn call(&self, #(#fn_args),*) #output {
                #(#arg_conversions)*
                #ret_stmt
            }
        }

        #abi_traits
    })
}

fn parse_item_impl(item_impl: &ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    if item_impl.trait_.is_some() {
        return Err(Error::new_spanned(
            item_impl,
            "#[ts_function] cannot be applied to trait impls",
        ));
    }

    let Type::Path(type_path) = &*item_impl.self_ty else {
        return Err(Error::new_spanned(
            &item_impl.self_ty,
            "Expected a simple path for the struct",
        ));
    };

    let struct_ident = type_path.path.get_ident().ok_or_else(|| {
        Error::new_spanned(
            &type_path.path,
            "Expected a single identifier for the struct",
        )
    })?;

    let method = item_impl
        .items
        .iter()
        .find_map(|item| {
            if let syn::ImplItem::Fn(method) = item
                && method.sig.ident == "call"
            {
                return Some(method);
            }
            None
        })
        .ok_or_else(|| Error::new_spanned(item_impl, "Missing `call` method in impl block"))?;

    let mut args = Vec::new();
    let mut inputs_iter = method.sig.inputs.iter();

    // Check first argument is `&self` or `&mut self`
    match inputs_iter.next() {
        Some(FnArg::Receiver(_)) => {}
        _ => {
            return Err(Error::new_spanned(
                &method.sig,
                "The `call` method must take `&self` or `&mut self` as its first parameter",
            ));
        }
    }

    for (i, arg) in inputs_iter.enumerate() {
        let FnArg::Typed(pat_type) = arg else {
            return Err(Error::new_spanned(arg, "Expected a typed argument"));
        };

        let ident = if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
            pat_ident.ident.clone()
        } else {
            format_ident!("arg{}", i)
        };

        args.push((ident, &*pat_type.ty));
    }

    let parsed = ParsedSignature {
        struct_ident,
        args,
        output: &method.sig.output,
    };

    let abi_traits = generate_abi_traits(&parsed)?;

    Ok(quote! {
        #item_impl
        #abi_traits
    })
}

fn type_to_ts(ty: &Type) -> syn::Result<String> {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().unwrap();
            let ident = &segment.ident;

            let ident_str = ident.to_string();
            match ident_str.as_str() {
                "f32" | "f64" | "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => {
                    Ok("number".to_string())
                }
                "i64" | "u64" | "isize" | "usize" => Ok("bigint".to_string()),
                "bool" => Ok("boolean".to_string()),
                "String" => Ok("string".to_string()),
                "JsValue" => Ok("any".to_string()),
                "Option" => {
                    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Option"));
                    };
                    let syn::GenericArgument::Type(inner_ty) =
                        args.args.first().ok_or_else(|| {
                            Error::new_spanned(ty, "Expected type argument for Option")
                        })?
                    else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Option"));
                    };
                    let inner_ts = type_to_ts(inner_ty)?;
                    Ok(format!("{} | undefined", inner_ts))
                }
                "Vec" => {
                    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Vec"));
                    };
                    let syn::GenericArgument::Type(inner_ty) = args
                        .args
                        .first()
                        .ok_or_else(|| Error::new_spanned(ty, "Expected type argument for Vec"))?
                    else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Vec"));
                    };
                    let inner_ts = type_to_ts(inner_ty)?;
                    Ok(format!("{}[]", inner_ts))
                }
                // Default to the Rust struct name for custom types (e.g. JS arrays, other types)
                _ => {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        let mut type_params = Vec::new();
                        for arg in &args.args {
                            if let syn::GenericArgument::Type(inner_ty) = arg {
                                type_params.push(type_to_ts(inner_ty)?);
                            } else {
                                type_params.push("any".to_string());
                            }
                        }
                        Ok(format!("{}<{}>", ident_str, type_params.join(", ")))
                    } else {
                        Ok(ident_str)
                    }
                }
            }
        }
        Type::Reference(type_ref) => {
            let inner_ty = &*type_ref.elem;

            // Handle `&str` special case
            if let Type::Path(type_path) = inner_ty
                && type_path.path.is_ident("str")
            {
                return Ok("string".to_string());
            }
            // Handle slice `&[T]`
            if let Type::Slice(type_slice) = inner_ty {
                let inner_ts = type_to_ts(&type_slice.elem)?;
                return Ok(format!("{}[]", inner_ts));
            }

            // Strip reference and map
            type_to_ts(inner_ty)
        }
        Type::ImplTrait(type_impl) => {
            // Find `Into<X>`
            for bound in &type_impl.bounds {
                if let syn::TypeParamBound::Trait(trait_bound) = bound
                    && let Some(segment) = trait_bound.path.segments.last()
                    && segment.ident == "Into"
                    && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    return type_to_ts(inner_ty);
                }
            }
            Err(Error::new_spanned(
                ty,
                "Unsupported `impl Trait`. Only `impl Into<T>` is supported.",
            ))
        }
        _ => Err(Error::new_spanned(
            ty,
            "Unsupported type for TypeScript mapping. Consider using `#[ts(type = \"...\")]` instead.",
        )),
    }
}

fn generate_abi_traits(parsed: &ParsedSignature) -> syn::Result<proc_macro2::TokenStream> {
    let struct_ident = parsed.struct_ident;
    let mut ts_args = Vec::new();

    for (ident, ty) in &parsed.args {
        let ts_ty = type_to_ts(ty)?;
        ts_args.push(format!("{}: {}", ident, ts_ty));
    }

    let ts_output = match parsed.output {
        ReturnType::Default => "void".to_string(),
        ReturnType::Type(_, ty) => type_to_ts(ty)?,
    };

    let ts_string = format!(
        "type {} = ({}) => {};",
        struct_ident,
        ts_args.join(", "),
        ts_output
    );

    let generated = quote! {
        #[::wasm_bindgen::prelude::wasm_bindgen(typescript_custom_section)]
        const _: &'static str = #ts_string;

        impl ::wasm_bindgen::describe::WasmDescribe for #struct_ident {
            fn describe() {
                <::js_sys::Function as ::wasm_bindgen::describe::WasmDescribe>::describe()
            }
        }

        impl ::wasm_bindgen::convert::FromWasmAbi for #struct_ident {
            type Abi = <::js_sys::Function as ::wasm_bindgen::convert::FromWasmAbi>::Abi;

            unsafe fn from_abi(js: Self::Abi) -> Self {
                Self(::js_sys::Function::from_abi(js))
            }
        }

        impl ::wasm_bindgen::convert::OptionFromWasmAbi for #struct_ident {
            fn is_none(abi: &Self::Abi) -> bool {
                <::js_sys::Function as ::wasm_bindgen::convert::OptionFromWasmAbi>::is_none(abi)
            }
        }

        impl From<::js_sys::Function> for #struct_ident {
            fn from(f: ::js_sys::Function) -> Self {
                Self(f)
            }
        }
    };

    Ok(generated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_type_to_ts() {
        assert_eq!(type_to_ts(&parse_quote!(f32)).unwrap(), "number");
        assert_eq!(type_to_ts(&parse_quote!(f64)).unwrap(), "number");
        assert_eq!(type_to_ts(&parse_quote!(u32)).unwrap(), "number");
        assert_eq!(type_to_ts(&parse_quote!(usize)).unwrap(), "bigint");
        assert_eq!(type_to_ts(&parse_quote!(bool)).unwrap(), "boolean");
        assert_eq!(type_to_ts(&parse_quote!(String)).unwrap(), "string");
        assert_eq!(type_to_ts(&parse_quote!(&str)).unwrap(), "string");
        assert_eq!(
            type_to_ts(&parse_quote!(Option<f64>)).unwrap(),
            "number | undefined"
        );
        assert_eq!(type_to_ts(&parse_quote!(Vec<f64>)).unwrap(), "number[]");
        assert_eq!(type_to_ts(&parse_quote!(&[u8])).unwrap(), "number[]");
        assert_eq!(
            type_to_ts(&parse_quote!(js_sys::Float64Array)).unwrap(),
            "Float64Array"
        );
        assert_eq!(type_to_ts(&parse_quote!(JsValue)).unwrap(), "any");
        assert_eq!(type_to_ts(&parse_quote!(impl Into<f64>)).unwrap(), "number");
        assert_eq!(
            type_to_ts(&parse_quote!(Result<String, String>)).unwrap(),
            "Result<string, string>"
        );
    }

    #[test]
    fn test_item_type() {
        let item_type: ItemType = parse_quote! {
            pub type OnClick = fn(x: f64, y: impl Into<f64>, arr: js_sys::Float64Array);
        };
        let result = parse_item_type(&item_type).unwrap();
        let result_str = result.to_string();

        assert!(
            result_str
                .contains("type OnClick = (x: number, y: number, arr: Float64Array) => void;")
        );
        assert!(result_str.contains("pub struct OnClick (pub :: js_sys :: Function) ;"));
        assert!(result_str.contains(
            "pub fn call (& self , x : f64 , y : impl Into < f64 > , arr : js_sys :: Float64Array)"
        ));
    }

    #[test]
    fn test_item_impl() {
        let item_impl: ItemImpl = parse_quote! {
            impl OnScroll {
                pub fn call(&self, y: f64) {
                    // body
                }
            }
        };
        let result = parse_item_impl(&item_impl).unwrap();
        let result_str = result.to_string();

        assert!(result_str.contains("type OnScroll = (y: number) => void;"));
        assert!(
            result_str.contains("impl :: wasm_bindgen :: describe :: WasmDescribe for OnScroll")
        );
    }
}

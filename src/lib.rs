use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, Ident, Item, ItemImpl, ItemType, PathArguments, ReturnType,
    Type, parse_macro_input,
};

#[macro_use]
mod ts_type;
mod ts_macro;

use crate::ts_type::ToTsType;

/// Generates TypeScript interface bindings from a Rust struct.
///
/// This attribute works identically to the upstream `ts-macro` attribute, allowing
/// the struct to define a TypeScript interface with property bindings seamlessly
/// mapped to Javascript functions.
///
/// It generates:
/// 1. A TypeScript interface string exposed as a custom wasm section
/// 2. Extensible bindings and trait implementations
///
/// The default behavior for field names is to convert to `camelCase` for Javascript conventions.
/// However, you can opt-out by adding `rename_all = "none"`:
///
/// ```rust,ignore
/// #[ts(rename_all = "none")]
/// struct MyStruct {
///     my_field_name: String, // Will remain "my_field_name" in TypeScript
/// }
/// ```
#[proc_macro_attribute]
pub fn ts(attr: TokenStream, input: TokenStream) -> TokenStream {
    ts_macro::ts(attr, input)
}

struct ParsedSignature<'a> {
    struct_ident: &'a Ident,
    args: Vec<(Ident, &'a Type)>,
    output: &'a ReturnType,
}

/// Generates TypeScript type aliases and `wasm-bindgen` ABI trait implementations
/// for Rust callback wrapper structs.
///
/// `ts-function` acts as a bridge for function/callback types in pure Rust when
/// interoperating with TypeScript using `ts-macro`. It can be applied to either
/// type aliases (`pub type MyCb = fn(args: ...)`) or `impl` blocks (the "escape hatch").
///
/// # Examples
///
/// **Basic Usage**
///
/// ```rust,ignore
/// use ts_function::{ts, ts_function};
///
/// #[ts_function]
/// pub type OnReadyCb = fn(msg: String);
///
/// #[ts]
/// struct AppCallbacks {
///     on_ready: OnReadyCb,
/// }
/// ```
///
/// **Escape Hatch Usage**
///
/// For completely custom serialization or embedding specific side-effects and error
/// handling directly into the callback execution:
///
/// ```rust,ignore
/// use wasm_bindgen::prelude::*;
/// use ts_function::ts_function;
///
/// pub struct CustomLoggingCallback(pub js_sys::Function);
///
/// #[ts_function]
/// impl CustomLoggingCallback {
///     pub fn call(&self, val: f64) {
///         // Call the JS function and handle errors internally
///         let _ = self.0.call1(
///             &wasm_bindgen::JsValue::NULL,
///             &wasm_bindgen::JsValue::from_f64(val),
///         );
///     }
/// }
/// ```
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

fn generate_return_conversion(ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().unwrap();
            let ident = &segment.ident;
            let ident_str = ident.to_string();

            if let Some(inner_ty) = get_slice_element_type(ty)
                && let Some(arr_type) = get_typed_array_ident(inner_ty)
            {
                return Ok(quote! {
                    let arr: ::js_sys::#arr_type = ::wasm_bindgen::JsCast::unchecked_into(res);
                    Ok(::std::convert::Into::<#ty>::into(arr.to_vec()))
                });
            }

            match ident_str.as_str() {
                "f32" | "f64" | "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => Ok(quote! {
                    res.as_f64().map(|v| v as #ty).ok_or_else(|| ::wasm_bindgen::JsValue::from_str("Expected a number"))
                }),
                "i64" | "u64" => Ok(quote! {
                    ::std::convert::TryInto::<#ty>::try_into(res).map_err(|_| ::wasm_bindgen::JsValue::from_str("Expected a BigInt"))
                }),
                "bool" => Ok(quote! {
                    res.as_bool().ok_or_else(|| ::wasm_bindgen::JsValue::from_str("Expected a boolean"))
                }),
                "String" => Ok(quote! {
                    res.as_string().ok_or_else(|| ::wasm_bindgen::JsValue::from_str("Expected a string"))
                }),
                "JsValue" => Ok(quote! {
                    Ok(res)
                }),
                "Option" => {
                    let PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return Err(Error::new_spanned(
                            ty,
                            "Expected generic argument for Option",
                        ));
                    };
                    let syn::GenericArgument::Type(inner_ty) = args.args.first().unwrap() else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Option"));
                    };
                    let inner_conversion = generate_return_conversion(inner_ty)?;
                    Ok(quote! {
                        if res.is_null() || res.is_undefined() {
                            Ok(None)
                        } else {
                            let res = { #inner_conversion };
                            res.map(Some)
                        }
                    })
                }
                _ => Ok(quote! {
                    Ok(::wasm_bindgen::JsCast::unchecked_into::<#ty>(res))
                }),
            }
        }
        _ => Err(Error::new_spanned(
            ty,
            "Unsupported return type in type alias pattern. Use the `impl` escape hatch instead.",
        )),
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
        let conversion = generate_conversion(ident, ty)?;
        arg_conversions.push(conversion);
        call_args.push(quote! { &#ident });
    }

    let args_len = call_args.len();
    if args_len > 9 {
        return Err(Error::new_spanned(
            item_type,
            "Functions with more than 9 arguments are not supported yet",
        ));
    }
    let call_method_name = format_ident!("call{}", args_len);
    let call_method = quote! { #call_method_name(&::wasm_bindgen::JsValue::NULL, #(#call_args),*) };

    let output = parsed.output;
    let (ret_type, ret_stmt) = match output {
        ReturnType::Default => (quote! { () }, quote! { self.0.#call_method.map(|_| ()) }),
        ReturnType::Type(_, ty) => {
            let conversion = generate_return_conversion(ty)?;
            (
                quote! { #ty },
                quote! {
                    let res = self.0.#call_method?;
                    #conversion
                },
            )
        }
    };

    Ok(quote! {
        pub struct #struct_ident(pub ::js_sys::Function);

        impl #struct_ident {
            pub fn call(&self, #(#fn_args),*) -> Result<#ret_type, ::wasm_bindgen::JsValue> {
                #(#arg_conversions)*
                #ret_stmt
            }
        }

        #abi_traits
    })
}

fn generate_conversion(ident: &Ident, ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    if let Type::ImplTrait(type_impl) = ty {
        for bound in &type_impl.bounds {
            if let syn::TypeParamBound::Trait(trait_bound) = bound
                && let Some(segment) = trait_bound.path.segments.last()
                && let PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(GenericArgument::Type(inner_ty)) = args.args.first()
            {
                match segment.ident.to_string().as_str() {
                    "Into" => {
                        let inner_conversion = generate_conversion(ident, inner_ty)?;
                        return Ok(quote! {
                            let #ident = ::std::convert::Into::<#inner_ty>::into(#ident);
                            #inner_conversion
                        });
                    }
                    "AsRef" => {
                        if let Type::Slice(slice) = inner_ty {
                            return Ok(generate_typed_array_conversion(ident, &slice.elem));
                        }
                    }
                    _ => {}
                }
            }
        }
        return Err(Error::new_spanned(
            ty,
            "Unsupported `impl Trait`. Only `impl Into<T>` and `impl AsRef<[T]>` are supported.",
        ));
    }

    if let Some(inner_ty) = get_slice_element_type(ty) {
        Ok(generate_typed_array_conversion(ident, inner_ty))
    } else {
        Ok(quote! {
            let #ident = ::std::convert::Into::<::wasm_bindgen::JsValue>::into(#ident);
        })
    }
}

fn generate_typed_array_conversion(ident: &Ident, inner_ty: &Type) -> proc_macro2::TokenStream {
    if let Some(arr_type) = get_typed_array_ident(inner_ty) {
        quote! {
            let #ident = ::wasm_bindgen::JsValue::from(::js_sys::#arr_type::from(::std::convert::AsRef::<[#inner_ty]>::as_ref(&#ident)));
        }
    } else {
        quote! {
            let #ident = ::wasm_bindgen::JsValue::from(
                ::std::convert::AsRef::<[#inner_ty]>::as_ref(&#ident)
                    .iter()
                    .map(::wasm_bindgen::JsValue::from)
                    .collect::<::js_sys::Array>()
            );
        }
    }
}

fn get_typed_array_ident(inner_ty: &Type) -> Option<proc_macro2::TokenStream> {
    let inner_str = match inner_ty {
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    };

    match inner_str.as_deref() {
        Some("u8") => Some(quote! { Uint8Array }),
        Some("i8") => Some(quote! { Int8Array }),
        Some("u16") => Some(quote! { Uint16Array }),
        Some("i16") => Some(quote! { Int16Array }),
        Some("u32") => Some(quote! { Uint32Array }),
        Some("i32") => Some(quote! { Int32Array }),
        Some("f32") => Some(quote! { Float32Array }),
        Some("f64") => Some(quote! { Float64Array }),
        Some("u64") => Some(quote! { BigUint64Array }),
        Some("i64") => Some(quote! { BigInt64Array }),
        _ => None,
    }
}

fn get_slice_element_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last()?;
            // Types that implement AsRef<[T]> and we can easily extract T from AST
            if matches!(
                segment.ident.to_string().as_str(),
                "Vec" | "Box" | "Arc" | "Rc"
            ) && let PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
            {
                if let Type::Slice(slice) = inner {
                    return Some(&*slice.elem);
                }
                return Some(inner);
            }
        }
        Type::Reference(type_ref) => {
            if let Type::Slice(type_slice) = &*type_ref.elem {
                return Some(&*type_slice.elem);
            }
            return get_slice_element_type(&type_ref.elem);
        }
        _ => {}
    }
    None
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

fn generate_abi_traits(parsed: &ParsedSignature) -> syn::Result<proc_macro2::TokenStream> {
    let struct_ident = parsed.struct_ident;
    let mut ts_args = Vec::new();

    for (ident, ty) in &parsed.args {
        let ts_ty = ty
            .to_ts_type()
            .map_err(|e| Error::new_spanned(ty, e.message))?
            .to_string();
        ts_args.push(format!("{}: {}", ident, ts_ty));
    }

    let ts_output = match parsed.output {
        ReturnType::Default => "void".to_string(),
        ReturnType::Type(_, ty) => ty
            .to_ts_type()
            .map_err(|e| Error::new_spanned(ty, e.message))?
            .to_string(),
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

    #[test]
    fn test_recursive_generics() {
        let item_type: ItemType = parse_quote! {
            pub type ResultCb = fn(res: Result<String, i32>);
        };
        let result = parse_item_type(&item_type).unwrap();
        let result_str = result.to_string();

        assert!(result_str.contains("type ResultCb = (res: Result<string, number>) => void;"));

        let item_type: ItemType = parse_quote! {
            pub type NestedVecCb = fn(args: Vec<Vec<f64>>);
        };
        let result = parse_item_type(&item_type).unwrap();
        let result_str = result.to_string();

        assert!(result_str.contains("type NestedVecCb = (args: Float64Array[]) => void;"));
    }
}

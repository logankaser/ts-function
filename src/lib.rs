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

/// Generates TypeScript type aliases and `wasm-bindgen` ABI trait implementations
/// for Rust items.
///
/// This attribute can be applied to:
/// 1. **Structs**: Generates a TypeScript interface and property bindings.
/// 2. **Enums**: Automatically applies `#[wasm_bindgen]` (supports C-style enums).
/// 3. **Type Aliases**: Generates a TypeScript type alias and a typed function wrapper.
/// 4. **Impl Blocks**: Generates a typed function wrapper for a `call` method.
///
/// # Examples
///
/// **Struct Usage**
///
/// ```rust,ignore
/// #[ts(rename_all = "camelCase")]
/// struct MyStruct {
///     field_name: String,
/// }
/// ```
///
/// **Enum Usage**
///
/// ```rust,ignore
/// #[ts]
/// enum Status { Active, Inactive }
/// ```
///
/// **Function Wrapper Usage**
///
/// ```rust,ignore
/// #[ts]
/// pub type OnReady = fn(msg: String);
///
/// #[ts]
/// struct AppFunctions {
///     on_ready: OnReady,
/// }
/// ```
#[proc_macro_attribute]
pub fn ts(attr: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as Item);
    ts_internal_dispatcher(attr.into(), item).into()
}

fn ts_internal_dispatcher(attr: proc_macro2::TokenStream, item: Item) -> proc_macro2::TokenStream {
    let attr_args = attr.clone();

    match &item {
        Item::Struct(item_struct) => {
            let args = match syn::parse2::<ts_macro::TsArgs>(attr_args) {
                Ok(args) => args,
                Err(err) => return err.to_compile_error(),
            };
            ts_macro::ts_internal(args, item_struct.clone())
        }
        Item::Enum(item_enum) => {
            let enum_name = &item_enum.ident;
            let variants = item_enum
                .variants
                .iter()
                .map(|variant| {
                    if !matches!(variant.fields, syn::Fields::Unit) {
                        return Err(Error::new_spanned(
                            variant,
                            "#[ts] enums must only contain unit variants",
                        ));
                    }
                    Ok(&variant.ident)
                })
                .collect::<syn::Result<Vec<_>>>();
            let variants = match variants {
                Ok(variants) => variants,
                Err(err) => return err.to_compile_error(),
            };
            let variant_constants = variants
                .iter()
                .map(|variant| {
                    let name = format_ident!(
                        "__TS_FUNCTION_VARIANT_{}",
                        variant.to_string().to_uppercase()
                    );
                    quote! { const #name: u32 = #enum_name::#variant as u32; }
                })
                .collect::<Vec<_>>();
            let conversion_arms = variants
                .iter()
                .map(|variant| {
                    let name = format_ident!(
                        "__TS_FUNCTION_VARIANT_{}",
                        variant.to_string().to_uppercase()
                    );
                    quote! { #name => ::std::result::Result::Ok(Self::#variant), }
                })
                .collect::<Vec<_>>();
            quote! {
                #[::wasm_bindgen::prelude::wasm_bindgen]
                #item_enum

                impl ::std::convert::TryFrom<::wasm_bindgen::JsValue> for #enum_name {
                    type Error = ::wasm_bindgen::JsValue;

                    #[inline]
                    fn try_from(value: ::wasm_bindgen::JsValue) -> ::std::result::Result<Self, Self::Error> {
                        let value = match value.as_f64() {
                            ::std::option::Option::Some(value) => value,
                            ::std::option::Option::None => {
                                return ::std::result::Result::Err(::wasm_bindgen::JsValue::from_str(
                                    concat!("Expected a number for enum ", stringify!(#enum_name)),
                                ));
                            }
                        };
                        if !value.is_finite()
                            || value.fract() != 0.0
                            || !(0.0..=u32::MAX as f64).contains(&value)
                        {
                            return ::std::result::Result::Err(::wasm_bindgen::JsValue::from_str(&format!(
                                "Invalid {} variant: {}", stringify!(#enum_name), value
                            )));
                        }
                        #(#variant_constants)*
                        match value as u32 {
                            #(#conversion_arms)*
                            _ => ::std::result::Result::Err(::wasm_bindgen::JsValue::from_str(&format!(
                                "Invalid {} variant: {}", stringify!(#enum_name), value
                            ))),
                        }
                    }
                }
            }
        }
        Item::Type(item_type) => match parse_item_type(item_type) {
            Ok(tokens) => tokens,
            Err(err) => err.to_compile_error(),
        },
        Item::Impl(item_impl) => match parse_item_impl(item_impl) {
            Ok(tokens) => tokens,
            Err(err) => err.to_compile_error(),
        },
        _ => Error::new_spanned(
            item,
            "#[ts] can only be applied to a struct, enum, type alias, or impl block",
        )
        .to_compile_error(),
    }
}

struct ParsedSignature<'a> {
    struct_ident: &'a Ident,
    args: Vec<(Ident, &'a Type)>,
    output: &'a ReturnType,
}

pub(crate) fn generate_try_convert_support(struct_ident: &syn::Ident) -> proc_macro2::TokenStream {
    let try_convert_name = format_ident!("try_convert_{}", struct_ident);
    let trait_name = format_ident!("IntoJsValue_{}", struct_ident);
    quote! {
        #[allow(non_camel_case_types)]
        trait #trait_name {
            fn into_js_value(self) -> ::wasm_bindgen::JsValue;
        }

        impl #trait_name for ::wasm_bindgen::JsValue {
            #[inline]
            fn into_js_value(self) -> ::wasm_bindgen::JsValue {
                self
            }
        }

        impl #trait_name for ::std::convert::Infallible {
            #[inline]
            fn into_js_value(self) -> ::wasm_bindgen::JsValue {
                match self {}
            }
        }

        #[inline]
        #[allow(non_snake_case)]
        fn #try_convert_name<T, E>(res: ::wasm_bindgen::JsValue) -> ::std::result::Result<T, ::wasm_bindgen::JsValue>
        where
            T: ::std::convert::TryFrom<::wasm_bindgen::JsValue, Error = E>,
            E: #trait_name,
        {
            ::std::convert::TryInto::<T>::try_into(res).map_err(#trait_name::into_js_value)
        }
    }
}

pub(crate) fn generate_return_conversion(
    struct_ident: &syn::Ident,
    ty: &Type,
) -> syn::Result<proc_macro2::TokenStream> {
    let try_convert_name = format_ident!("try_convert_{}", struct_ident);
    match ty {
        Type::Path(type_path) => {
            let segment = type_path
                .path
                .segments
                .last()
                .ok_or_else(|| Error::new_spanned(ty, "Expected a type segment"))?;
            let ident = &segment.ident;
            let ident_str = ident.to_string();

            if let Some(inner_ty) = get_slice_element_type(ty)
                && let Some(arr_type) = get_typed_array_ident(inner_ty)
            {
                return Ok(quote! {
                    let arr: ::js_sys::#arr_type = ::wasm_bindgen::JsCast::dyn_into(res)
                        .map_err(|_| ::wasm_bindgen::JsValue::from_str(concat!("Expected a ", stringify!(#arr_type))))?;
                    ::std::result::Result::Ok::<_, ::wasm_bindgen::JsValue>(::std::convert::Into::<#ty>::into(arr.to_vec()))
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
                    ::std::result::Result::Ok::<_, ::wasm_bindgen::JsValue>(res)
                }),
                "Option" => {
                    let PathArguments::AngleBracketed(args) = &segment.arguments else {
                        return Err(Error::new_spanned(
                            ty,
                            "Expected generic argument for Option",
                        ));
                    };
                    let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() else {
                        return Err(Error::new_spanned(ty, "Expected type argument for Option"));
                    };
                    let inner_conversion = generate_return_conversion(struct_ident, inner_ty)?;
                    Ok(quote! {
                        if res.is_null() || res.is_undefined() {
                            ::std::result::Result::Ok::<_, ::wasm_bindgen::JsValue>(None)
                        } else {
                            let res = { #inner_conversion };
                            res.map(Some)
                        }
                    })
                }
                _ => Ok(quote! {
                    #try_convert_name::<#ty, _>(res)
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
    item_type.modifiers.require_empty()?;

    let Type::FnPtr(bare_fn) = &*item_type.ty else {
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
            let conversion = generate_return_conversion(struct_ident, ty)?;
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

        const _: () = {
            #abi_traits

            impl #struct_ident {
                pub fn call(&self, #(#fn_args),*) -> Result<#ret_type, ::wasm_bindgen::JsValue> {
                    #(#arg_conversions)*
                    #ret_stmt
                }
            }
        };
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
    item_impl.modifiers.require_empty()?;

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

    let try_convert_support = generate_try_convert_support(struct_ident);

    let generated = quote! {
        #[::wasm_bindgen::prelude::wasm_bindgen(typescript_custom_section)]
        const _: &'static str = #ts_string;

        #try_convert_support

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

        impl ::std::convert::TryFrom<::wasm_bindgen::JsValue> for #struct_ident {
            type Error = ::wasm_bindgen::JsValue;

            #[inline]
            fn try_from(value: ::wasm_bindgen::JsValue) -> ::std::result::Result<Self, Self::Error> {
                use ::wasm_bindgen::JsCast;
                let f = value.dyn_into::<::js_sys::Function>()?;
                ::std::result::Result::Ok(Self(f))
            }
        }

        impl From<#struct_ident> for ::wasm_bindgen::JsValue {
            fn from(f: #struct_ident) -> Self {
                ::wasm_bindgen::JsValue::from(f.0)
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
    fn test_dispatcher_item_struct() {
        let input: Item = parse_quote! {
            pub struct MyStruct {
                pub field: f64,
            }
        };
        let attr = quote! {};
        let result = ts_internal_dispatcher(attr, input);
        let result_str = result.to_string();

        assert!(result_str.contains("export interface MyStruct"));
        assert!(result_str.contains("field: number;"));
    }

    #[test]
    fn test_dispatcher_item_type() {
        let input: Item = parse_quote! {
            pub type OnClick = fn(x: f64);
        };
        let attr = quote! {};
        let result = ts_internal_dispatcher(attr, input);
        let result_str = result.to_string();

        assert!(result_str.contains("type OnClick = (x: number) => void;"));
        assert!(result_str.contains("pub struct OnClick (pub :: js_sys :: Function) ;"));
    }

    #[test]
    fn test_dispatcher_item_impl() {
        let input: Item = parse_quote! {
            impl OnScroll {
                pub fn call(&self, y: f64) {}
            }
        };
        let attr = quote! {};
        let result = ts_internal_dispatcher(attr, input);
        let result_str = result.to_string();

        assert!(result_str.contains("type OnScroll = (y: number) => void;"));
        assert!(
            result_str.contains("impl :: wasm_bindgen :: describe :: WasmDescribe for OnScroll")
        );
    }

    #[test]
    fn test_enum_item() {
        let input: Item = parse_quote! {
            pub enum Status { Active, Inactive }
        };
        let attr = quote! {};
        let result = ts_internal_dispatcher(attr, input);
        let result_str = result.to_string();

        assert!(result_str.contains("# [:: wasm_bindgen :: prelude :: wasm_bindgen]"));
        assert!(result_str.contains("pub enum Status { Active , Inactive }"));
    }

    #[test]
    fn test_recursive_generics() {
        let item_type: ItemType = parse_quote! {
            pub type ResultFn = fn(res: Result<String, i32>);
        };
        let result = parse_item_type(&item_type).unwrap();
        let result_str = result.to_string();

        assert!(result_str.contains("type ResultFn = (res: Result<string, number>) => void;"));

        let item_type: ItemType = parse_quote! {
            pub type NestedVecFn = fn(args: Vec<Vec<f64>>);
        };
        let result = parse_item_type(&item_type).unwrap();
        let result_str = result.to_string();

        assert!(result_str.contains("type NestedVecFn = (args: Float64Array[]) => void;"));
    }

    #[test]
    fn test_item_impl_rejects_modifiers() {
        let item_impl: ItemImpl = parse_quote! {
            default impl Callback {
                fn call(&self) {}
            }
        };

        assert!(parse_item_impl(&item_impl).is_err());
    }
}

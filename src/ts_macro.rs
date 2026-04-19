// This file contains code adapted from the wasm-utils-rs project
// (https://github.com/ryangoree/wasm-utils-rs).
//
// Original Copyright 2024 DELV, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::ts_type::{ToTsType, TsType};
use heck::{ToLowerCamelCase, ToPascalCase};
use quote::{format_ident, quote};
use syn::{
    Error, Fields, FieldsNamed, Ident, ItemStruct, Meta, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

pub(crate) struct TsArgs {
    name: Option<Ident>,
    extends: Option<Punctuated<Ident, Token![,]>>,
    rename_all: Option<String>,
}

impl Parse for TsArgs {
    fn parse(input: ParseStream) -> Result<Self, Error> {
        let mut args = TsArgs {
            name: None,
            extends: None,
            rename_all: None,
        };

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => args.name = Some(input.parse()?),
                "extends" => args.extends = Some(input.parse_terminated(Ident::parse, Token![,])?),
                "rename_all" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }) = input.parse()?
                    {
                        args.rename_all = Some(lit_str.value());
                    } else {
                        return Err(Error::new(
                            key.span(),
                            "Expected string literal for `rename_all`",
                        ));
                    }
                }
                _ => {
                    return Err(Error::new(
                        key.span(),
                        format!("Unknown argument: `{}`", key),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

/// Generate TypeScript interface bindings from a Rust struct.
pub fn ts_internal(args: TsArgs, item: ItemStruct) -> proc_macro2::TokenStream {
    // Ensure the input is a struct with named fields
    let (struct_name, fields) = match &item {
        ItemStruct {
            ident,
            fields: Fields::Named(fields),
            ..
        } => (ident, fields),
        _ => {
            return quote! {
                compile_error!("The `ts` attribute can only be used on structs with named fields.");
            };
        }
    };

    let binding_name = match args.name {
        Some(name) => format_ident!("{}", name),
        None => format_ident!("I{}", struct_name),
    };
    let ts_interface_name = struct_name.to_string();
    let mut ts_fields = vec![];
    let mut field_conversions = vec![];
    let mut field_getters = vec![];
    let mut field_setters = vec![];
    let mut processed_fields = vec![];

    // Iterate over the fields of the struct to generate entries for the
    // TypeScript interface and the field conversions
    for field in &fields.named {
        let field_type = &field.ty;
        let field_name = field.ident.as_ref().unwrap();
        let mut field = field.clone();
        let mut doc_lines = vec![];
        let mut is_optional = false;

        // Convert the Rust field name to a TypeScript field name
        let mut ts_field_name = match args.rename_all.as_deref() {
            Some("none") => format_ident!("{}", field_name),
            _ => format_ident!("{}", field_name.to_string().to_lower_camel_case()),
        };

        // Convert the Rust type to a TypeScript type
        let mut ts_field_type = match field_type.to_ts_type() {
            Ok(ts_type) => {
                // if the type is `undefined` or unioned with `undefined`, make
                // it optional
                let undefined = TsType::Base("undefined".to_string());
                if ts_type == undefined || ts_type.is_union_with(&undefined) {
                    is_optional = true;
                }

                ts_type
            }
            Err(err) => {
                let msg = format!("{}", err);
                return quote! { compile_error!(#msg); };
            }
        };

        // Iterate over the attributes of the field to extract the `ts`
        // attribute and doc comments
        let mut i = 0;
        while i < field.attrs.len() {
            let attr = &field.attrs[i];

            // Collect doc comments
            if attr.path().is_ident("doc") {
                if let Meta::NameValue(syn::MetaNameValue {
                    value:
                        syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(lit_str),
                            ..
                        }),
                    ..
                }) = &attr.meta
                {
                    doc_lines.push(lit_str.value());
                }
                field.attrs.remove(i);
                continue;
            }

            if !attr.path().is_ident("ts") {
                i += 1;
                continue;
            }

            // Parse the `ts` attribute arguments
            match &attr.meta {
                Meta::List(list) => {
                    let result =
                        list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated);
                    match result {
                        Ok(nested) => {
                            for arg in nested {
                                if let Meta::NameValue(nv) = arg {
                                    let key = match nv.path.get_ident() {
                                        Some(ident) => ident.to_string(),
                                        None => {
                                            let msg = "Expected an identifier for the key";
                                            return quote! { compile_error!(#msg); };
                                        }
                                    };
                                    match key.as_str() {
                                        "name" => {
                                            if let syn::Expr::Lit(syn::ExprLit {
                                                lit: syn::Lit::Str(lit_str),
                                                ..
                                            }) = nv.value
                                            {
                                                ts_field_name =
                                                    format_ident!("{}", lit_str.value());
                                            } else {
                                                let msg = format!(
                                                    "`name` for field `{field_name}` must be a string literal."
                                                );
                                                return quote! { compile_error!(#msg); };
                                            }
                                        }
                                        "type" => {
                                            if let syn::Expr::Lit(syn::ExprLit {
                                                lit: syn::Lit::Str(lit_str),
                                                ..
                                            }) = nv.value
                                            {
                                                match TsType::from_ts_str(lit_str.value().as_str())
                                                {
                                                    Ok(ts_type) => ts_field_type = ts_type,
                                                    Err(err) => {
                                                        let msg = format!("{}", err);
                                                        return quote! { compile_error!(#msg); };
                                                    }
                                                }
                                            } else {
                                                let msg = format!(
                                                    "`type` for field `{field_name}` must be a string literal."
                                                );
                                                return quote! { compile_error!(#msg); };
                                            }
                                        }
                                        "optional" => {
                                            if let syn::Expr::Lit(syn::ExprLit {
                                                lit: syn::Lit::Bool(bool_lit),
                                                ..
                                            }) = nv.value
                                            {
                                                is_optional = bool_lit.value;
                                            } else {
                                                let msg = format!(
                                                    "`optional` for field `{field_name}` must be a boolean literal."
                                                );
                                                return quote! { compile_error!(#msg); };
                                            }
                                        }
                                        unknown => {
                                            let msg = format!(
                                                r#"Unknown argument for field `{field}`: `{attr}`. Options are:
                                    - type: The TypeScript type of the field
                                    - name: The name of the field in the TypeScript interface
                                    - optional: Whether the field is optional in TypeScript"#,
                                                field = field_name,
                                                attr = unknown
                                            );
                                            return quote! { compile_error!(#msg); };
                                        }
                                    }
                                } else {
                                    let msg = format!(
                                        "`ts` attribute for field `{}` must be a list of name-value pairs, e.g. `#[ts(type = \"{}\")]`.",
                                        field_name,
                                        field_name.to_string().to_pascal_case()
                                    );
                                    return quote! { compile_error!(#msg); };
                                }
                            }
                        }
                        Err(err) => {
                            let msg = format!("{}", err);
                            return quote! { compile_error!(#msg); };
                        }
                    }
                }
                _ => {
                    let msg = format!(
                        "`ts` attribute for field `{}` must be a list, e.g. `#[ts(type = \"Js{}\")]`.",
                        field_name,
                        field_name.to_string().to_pascal_case(),
                    );
                    return quote! { compile_error!(#msg); };
                }
            }

            // Remove the attribute from the field
            field.attrs.remove(i);
        }

        // Add an entry for the TypeScript interface
        let optional_char = match is_optional {
            true => "?",
            false => "",
        };
        let ts_doc_comment = match doc_lines.is_empty() {
            true => "".to_string(),
            false => format!("/**\n   *{}\n   */\n  ", doc_lines.join("\n   *")),
        };
        ts_fields.push(format!(
            "{ts_doc_comment}{ts_field_name}{optional_char}: {ts_field_type};"
        ));

        // Add a getter for the field to the binding
        let rs_doc_comment = doc_lines.iter().map(|line| quote! { #[doc = #line] });
        field_getters.push(quote! {
            #(#rs_doc_comment)*
            #[::wasm_bindgen::prelude::wasm_bindgen(method, getter = #ts_field_name)]
            pub fn #field_name(this: &#binding_name) -> #field_type;
        });

        // Add an entry for the `From` implementation
        field_conversions.push(quote! {
            #field_name: js_value.#field_name()
        });

        // Add a setter for the `Into<JsValue>` implementation
        let ts_field_name_str = ts_field_name.to_string();
        field_setters.push(quote! {
            ::js_sys::Reflect::set(
                &obj,
                &::wasm_bindgen::JsValue::from_str(#ts_field_name_str),
                &value.#field_name.into()
            ).unwrap();
        });

        // Add the processed field to the struct
        processed_fields.push(field);
    }

    // Generate the TypeScript interface definition
    let const_name = format_ident!("{}__TS_DEF", struct_name.to_string().to_uppercase());
    let (extends_clause, extends) = match args.extends {
        Some(extends) => (
            format!(
                " extends {}",
                extends
                    .iter()
                    .map(|base| base.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            extends.into_iter().collect(),
        ),
        None => ("".to_string(), vec![]),
    };
    let ts_definition = format!(
        r#"export interface {ts_interface_name}{extends_clause} {{
  {}
}}"#,
        ts_fields.join("\n  ")
    );

    // Prep the expanded struct with the processed attributes removed
    let processed_struct = ItemStruct {
        fields: Fields::Named(FieldsNamed {
            named: Punctuated::from_iter(processed_fields),
            brace_token: fields.brace_token,
        }),
        ..item.clone()
    };

    let expanded = quote! {
        #[::wasm_bindgen::prelude::wasm_bindgen(typescript_custom_section)]
        const #const_name: &'static str = #ts_definition;

        #[::wasm_bindgen::prelude::wasm_bindgen]
        extern "C" {
            #[derive(Clone)]
            #[wasm_bindgen(typescript_type = #ts_interface_name, #(extends = #extends),*)]
            pub type #binding_name;

            #(#field_getters)*
        }

        impl ::wasm_bindgen::describe::WasmDescribe for #struct_name {
            fn describe() {
                <::wasm_bindgen::JsValue as ::wasm_bindgen::describe::WasmDescribe>::describe()
            }
        }

        impl ::wasm_bindgen::convert::FromWasmAbi for #struct_name {
            type Abi = <::wasm_bindgen::JsValue as ::wasm_bindgen::convert::FromWasmAbi>::Abi;
            #[inline]
            unsafe fn from_abi(js: Self::Abi) -> Self {
                let js_value = unsafe { <::wasm_bindgen::JsValue as ::wasm_bindgen::convert::FromWasmAbi>::from_abi(js) };
                ::std::convert::Into::<Self>::into(js_value)
            }
        }

        impl ::wasm_bindgen::convert::IntoWasmAbi for #struct_name {
            type Abi = <::wasm_bindgen::JsValue as ::wasm_bindgen::convert::IntoWasmAbi>::Abi;
            #[inline]
            fn into_abi(self) -> Self::Abi {
                let js_value: ::wasm_bindgen::JsValue = ::std::convert::Into::<::wasm_bindgen::JsValue>::into(self);
                js_value.into_abi()
            }
        }

        impl ::wasm_bindgen::convert::OptionFromWasmAbi for #struct_name {
            #[inline]
            fn is_none(abi: &Self::Abi) -> bool {
                <::wasm_bindgen::JsValue as ::wasm_bindgen::convert::OptionFromWasmAbi>::is_none(abi)
            }
        }

        impl ::wasm_bindgen::convert::OptionIntoWasmAbi for #struct_name {
            #[inline]
            fn none() -> Self::Abi {
                <::wasm_bindgen::JsValue as ::wasm_bindgen::convert::OptionIntoWasmAbi>::none()
            }
        }

        impl From<#binding_name> for #struct_name {
            /// Convert the JS binding into the Rust struct
            fn from(js_value: #binding_name) -> Self {
                js_value.parse()
            }
        }

        impl From<::wasm_bindgen::JsValue> for #struct_name {
            fn from(js_value: ::wasm_bindgen::JsValue) -> Self {
                use ::wasm_bindgen::JsCast;
                js_value.unchecked_into::<#binding_name>().parse()
            }
        }

        impl From<#struct_name> for ::wasm_bindgen::JsValue {
            fn from(value: #struct_name) -> Self {
                let obj = ::js_sys::Object::new();
                #( #field_setters )*
                ::wasm_bindgen::JsValue::from(obj)
            }
        }

        impl From<#struct_name> for #binding_name {
            fn from(value: #struct_name) -> Self {
                use ::wasm_bindgen::JsCast;
                ::wasm_bindgen::JsValue::from(value).unchecked_into::<#binding_name>()
            }
        }

        impl #binding_name {
            /// Parse the JS binding into its Rust struct
            pub fn parse(&self) -> #struct_name {
                let js_value = self;
                #struct_name {
                    #(#field_conversions),*
                }
            }
        }

        #[allow(unused)]
        #[doc = "### Typescript Binding"]
        #[doc = ""]
        #[doc = "Below is the TypeScript definition for the binding generated by the `ts` attribute."]
        #[doc = ""]
        #[doc = "```ts"]
        #[doc = #ts_definition]
        #[doc = "```"]
        #[doc = ""]
        #processed_struct
    };

    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ts_args_parse() {
        let attr = quote! { name = MyStruct, rename_all = "none", extends = Base1, Base2 };
        let args: TsArgs = syn::parse2(attr).unwrap();
        assert_eq!(args.name.unwrap().to_string(), "MyStruct");
        assert_eq!(args.extends.unwrap().len(), 2);
        assert_eq!(args.rename_all.unwrap(), "none");
    }

    #[test]
    fn test_ts_enum_field() {
        let attr = quote! {};
        let input = quote! {
            pub struct User {
                pub status: Status,
            }
        };
        let args: TsArgs = syn::parse2(attr).unwrap();
        let item: ItemStruct = syn::parse2(input).unwrap();
        let result = ts_internal(args, item);
        let result_str = result.to_string();

        assert!(result_str.contains("export interface User"));
        assert!(result_str.contains("status: Status;"));
    }
}

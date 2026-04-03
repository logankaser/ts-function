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
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Error, Fields, FieldsNamed, Ident, ItemStruct, Meta, Token,
};

/// Return a [`TokenStream`] that expands into a formatted [`compile_error!`].
///
/// [`compile_error!`]: https://doc.rust-lang.org/std/macro.compile_error.html
macro_rules! abort {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        return TokenStream::from(quote! {
            compile_error!(#msg);
        });
    }};
}

struct TsArgs {
    name: Option<Ident>,
    extends: Option<Punctuated<Ident, Token![,]>>,
}

impl Parse for TsArgs {
    fn parse(input: ParseStream) -> Result<Self, Error> {
        let mut args = TsArgs {
            name: None,
            extends: None,
        };

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => args.name = Some(input.parse()?),
                "extends" => args.extends = Some(input.parse_terminated(Ident::parse, Token![,])?),
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
pub fn ts(attr: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as TsArgs);
    let item = parse_macro_input!(input as ItemStruct);

    // Ensure the input is a struct with named fields
    let (struct_name, fields) = match &item {
        ItemStruct {
            ident,
            fields: Fields::Named(fields),
            ..
        } => (ident, fields),
        _ => abort!("The `ts` attribute can only be used on structs with named fields."),
    };

    let ts_name = match args.name {
        Some(name) => format_ident!("{}", name),
        None => format_ident!("I{}", struct_name),
    };
    let mut ts_fields = vec![];
    let mut field_conversions = vec![];
    let mut field_getters = vec![];
    let mut processed_fields = vec![];

    // Iterate over the fields of the struct to generate entries for the
    // TypeScript interface and the field conversions
    for field in &fields.named {
        let field_type = &field.ty;
        let field_name = field.ident.as_ref().unwrap();
        let mut field = field.clone();
        let mut doc_lines = vec![];
        let mut is_optional = false;

        // Convert the Rust field name to a camelCase TypeScript field name
        let mut ts_field_name = format_ident!("{}", field_name.to_string().to_lower_camel_case());

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
            Err(err) => abort!("{}", err),
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
                                    let key = nv.path.get_ident().unwrap().to_string();
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
                                                abort!(
                                                    "`name` for field `{field_name}` must be a string literal."
                                                );
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
                                                    Err(err) => abort!("{}", err),
                                                }
                                            } else {
                                                abort!(
                                                    "`type` for field `{field_name}` must be a string literal."
                                                );
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
                                                abort!(
                                                    "`optional` for field `{field_name}` must be a boolean literal."
                                                );
                                            }
                                        }
                                        unknown => abort!(
                                            r#"Unknown argument for field `{field}`: `{attr}`. Options are:
                                    - type: The TypeScript type of the field
                                    - name: The name of the field in the TypeScript interface
                                    - optional: Whether the field is optional in TypeScript"#,
                                            field = field_name.to_string(),
                                            attr = unknown
                                        ),
                                    }
                                } else {
                                    abort!(
                                        "`ts` attribute for field `{}` must be a list of name-value pairs, e.g. `#[ts(type = \"{}\")]`.",
                                        field_name.to_string(),
                                        field_name.to_string().to_pascal_case()
                                    );
                                }
                            }
                        }
                        Err(err) => abort!("{}", err),
                    }
                }
                _ => {
                    abort!(
                        "`ts` attribute for field `{}` must be a list, e.g. `#[ts(type = \"Js{}\")]`.",
                        field_name.to_string(),
                        field_name.to_string().to_pascal_case(),
                    )
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
            pub fn #field_name(this: &#ts_name) -> #field_type;
        });

        // Add an entry for the `From` implementation
        field_conversions.push(quote! {
            #field_name: js_value.#field_name()
        });

        // Add the processed field to the struct
        processed_fields.push(field);
    }

    // Generate the TypeScript interface definition
    let const_name = format_ident!("{}", &ts_name.to_string().to_uppercase());
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
        r#"interface {ts_name}{extends_clause} {{
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
            #[wasm_bindgen(typescript_type = #ts_name, #(extends = #extends),*)]
            pub type #ts_name;

            #(#field_getters)*
        }

        impl From<#ts_name> for #struct_name {
            /// Convert the JS binding into the Rust struct
            fn from(js_value: #ts_name) -> Self {
                js_value.parse()
            }
        }

        impl #ts_name {
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

    TokenStream::from(expanded)
}

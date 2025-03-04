// Proc macros to automatically derive try_from for typed enums
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(TryFromPrimitive)]
pub fn derive_try_from_primitive(input: TokenStream) {
    // parse input tokens into syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    let enum_name = input.ident;

    let data_enum = if let syn::Data::Enum(data_enum) = input.data {
        data_enum
    } else {
        return syn::Error::new_spanned(
            input.ident,
            "TryFromPmitive can only be derived for enums",
        )
        .to_compile_error()
        .into();
    };

    // Default discriminant type is isize if #[repr(T)] is not specified.
    let mut repr_type = quote!(isize);

    for attr in input.attrs.iter() {
        if attr.path().is_ident("repr") {
            attr.parse_nested_meta(|meta| {
                if let Some(ident) = meta.path.get_ident() {
                    match ident.to_string().as_str() {
                        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64"
                        | "isize" => {
                            repr_type = quote!(#ident);
                        }
                        _ => {}
                    }
                }
                Ok(())
            });
        }
    }
}

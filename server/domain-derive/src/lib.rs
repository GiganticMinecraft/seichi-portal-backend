use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(UnsafeFromRawParts)]
pub fn derive_unsafe_from_raw_parts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_unsafe_from_raw_parts(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand_unsafe_from_raw_parts(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let generics = input.generics;

    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "UnsafeFromRawParts can only be derived for structs",
        ));
    };

    let Fields::Named(fields) = data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "UnsafeFromRawParts can only be derived for structs with named fields",
        ));
    };

    let field_names = fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().expect("named field"));
    let field_args = fields.named.iter().map(|field| {
        let name = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        quote! { #name: #ty }
    });

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// 永続化済みのフィールド値から復元します。
            ///
            /// # Safety
            /// 新規作成ではなく、データベースなど信頼できる永続化済みデータの復元にのみ使用してください。
            #[allow(clippy::too_many_arguments)]
            pub unsafe fn from_raw_parts(#(#field_args),*) -> Self {
                Self {
                    #(#field_names),*
                }
            }
        }
    })
}

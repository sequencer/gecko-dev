/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use proc_macro2::Ident;
use quote::ToTokens;
use syn::{punctuated::Punctuated, Token};
use uniffi_meta::{FnParamMetadata, Type};

pub(super) fn fn_param_metadata(
    params: &Punctuated<syn::FnArg, Token![,]>,
) -> syn::Result<Vec<FnParamMetadata>> {
    params
        .iter()
        .filter_map(|a| {
            let _is_method = false;
            let (name, ty) = match a {
                syn::FnArg::Receiver(_) => return None,
                syn::FnArg::Typed(pat_ty) => {
                    let name = match &*pat_ty.pat {
                        syn::Pat::Ident(pat_id) => pat_id.ident.to_string(),
                        _ => unimplemented!(),
                    };
                    (name, &pat_ty.ty)
                }
            };

            Some(convert_type(ty).map(|ty| FnParamMetadata { name, ty }))
        })
        .collect()
}

pub(super) fn return_type_metadata(ty: &syn::ReturnType) -> syn::Result<Option<Type>> {
    Ok(match ty {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => Some(convert_type(ty)?),
    })
}

pub(crate) fn convert_type(ty: &syn::Type) -> syn::Result<Type> {
    let type_path = type_as_type_path(ty)?;

    if type_path.qself.is_some() {
        return Err(syn::Error::new_spanned(
            type_path,
            "qualified self types are not currently supported by uniffi::export",
        ));
    }

    if type_path.path.segments.len() > 1 {
        return Err(syn::Error::new_spanned(
            type_path,
            "qualified paths in types are not currently supported by uniffi::export",
        ));
    }

    match &type_path.path.segments.first() {
        Some(seg) => match &seg.arguments {
            syn::PathArguments::None => convert_bare_type_name(&seg.ident),
            syn::PathArguments::AngleBracketed(a) => convert_generic_type(&seg.ident, a),
            syn::PathArguments::Parenthesized(_) => Err(type_not_supported(type_path)),
        },
        None => Err(syn::Error::new_spanned(
            type_path,
            "unreachable: TypePath must have non-empty segments",
        )),
    }
}

fn convert_generic_type(
    ident: &Ident,
    a: &syn::AngleBracketedGenericArguments,
) -> syn::Result<Type> {
    let mut it = a.args.iter();
    match it.next() {
        // `u8<>` is a valid way to write `u8` in the type namespace, so why not?
        None => convert_bare_type_name(ident),
        Some(arg1) => match it.next() {
            None => convert_generic_type1(ident, arg1),
            Some(arg2) => match it.next() {
                None => convert_generic_type2(ident, arg1, arg2),
                Some(_) => Err(syn::Error::new_spanned(
                    ident,
                    "types with more than two generics are not currently
                     supported by uniffi::export",
                )),
            },
        },
    }
}

fn convert_bare_type_name(ident: &Ident) -> syn::Result<Type> {
    match ident.to_string().as_str() {
        "u8" => Ok(Type::U8),
        "u16" => Ok(Type::U16),
        "u32" => Ok(Type::U32),
        "u64" => Ok(Type::U64),
        "i8" => Ok(Type::I8),
        "i16" => Ok(Type::I16),
        "i32" => Ok(Type::I32),
        "i64" => Ok(Type::I64),
        "f32" => Ok(Type::F32),
        "f64" => Ok(Type::F64),
        "bool" => Ok(Type::Bool),
        "String" => Ok(Type::String),
        _ => Err(type_not_supported(ident)),
    }
}

fn convert_generic_type1(ident: &Ident, arg: &syn::GenericArgument) -> syn::Result<Type> {
    let arg = arg_as_type(arg)?;
    match ident.to_string().as_str() {
        "Arc" => Ok(Type::ArcObject {
            object_name: type_as_type_path(arg)?
                .path
                .get_ident()
                .ok_or_else(|| type_not_supported(arg))?
                .to_string(),
        }),
        "Option" => Ok(Type::Option {
            inner_type: convert_type(arg)?.into(),
        }),
        "Vec" => Ok(Type::Vec {
            inner_type: convert_type(arg)?.into(),
        }),
        _ => Err(type_not_supported(ident)),
    }
}

fn convert_generic_type2(
    ident: &Ident,
    arg1: &syn::GenericArgument,
    arg2: &syn::GenericArgument,
) -> syn::Result<Type> {
    let arg1 = arg_as_type(arg1)?;
    let arg2 = arg_as_type(arg2)?;

    match ident.to_string().as_str() {
        "HashMap" => Ok(Type::HashMap {
            key_type: convert_type(arg1)?.into(),
            value_type: convert_type(arg2)?.into(),
        }),
        _ => Err(type_not_supported(ident)),
    }
}

pub(super) fn type_as_type_path(ty: &syn::Type) -> syn::Result<&syn::TypePath> {
    match ty {
        syn::Type::Group(g) => type_as_type_path(&g.elem),
        syn::Type::Paren(p) => type_as_type_path(&p.elem),
        syn::Type::Path(p) => Ok(p),
        _ => Err(type_not_supported(ty)),
    }
}

fn arg_as_type(arg: &syn::GenericArgument) -> syn::Result<&syn::Type> {
    match arg {
        syn::GenericArgument::Type(t) => Ok(t),
        _ => Err(syn::Error::new_spanned(
            arg,
            "non-type generic parameters are not currently supported by uniffi::export",
        )),
    }
}

fn type_not_supported(ty: &impl ToTokens) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        "this type is not currently supported by uniffi::export in this position",
    )
}

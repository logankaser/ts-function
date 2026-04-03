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

use std::{fmt, panic::Location, str::FromStr};

// Internal macros for ts-type logic

macro_rules! type_error_at {
  ($location:expr, $($arg:tt)*) => {
      crate::ts_type::TsTypeError {
          message: format!($($arg)*),
          location: $location.to_string(),
      }
  };
}

macro_rules! ts_tt {
  (( $single:tt )) => {{ ts_tt!($single) }};
  (( # $var:ident ))  => {{ $var.clone() }};
  (( $($items:tt)+ )) => {{
      let inner = ts_type!($($items)*);
      match inner {
          crate::ts_type::TsType::Intersection(_) => crate::ts_type::TsType::Paren(Box::new(inner)),
          crate::ts_type::TsType::Union(_) => crate::ts_type::TsType::Paren(Box::new(inner)),
          _ => inner,
      }
  }};
  ([ $first:tt $($items:tt)* ]) => {{
      let tuple = crate::ts_type::TsType::Tuple(vec![]);
      let mut stack = vec![tuple];
      let mut first = ts_tt!($first);
      build_ts_type!(stack first $($items)*)
  }};
  ($base:tt) => {{ crate::ts_type::TsType::Base(stringify!($base).to_string()) }};
}

macro_rules! build_ts_type {
  ($stack:ident $ty:ident [] $($rest:tt)*) => {{
      $ty = $ty.in_array();
      build_ts_type!($stack $ty $($rest)*)
  }};
  ($stack:ident $ty:ident [ $($keys:tt)+ ] $($rest:tt)*) => {{
      $ty = $ty.property(Box::new(ts_type!($($keys)+)));
      build_ts_type!($stack $ty $($rest)*)
  }};
  ($stack:ident $ty:ident & $other:tt $($rest:tt)*) => {{
      let intersection = crate::ts_type::TsType::Intersection(vec![$ty]);
      $stack.push(intersection);
      $ty = ts_tt!($other);
      build_ts_type!($stack $ty $($rest)*)
  }};
  ($stack:ident $ty:ident | $other:tt $($rest:tt)*) => {{
      let _union = crate::ts_type::TsType::Union(vec![$ty]);
      $stack.push(_union);
      $ty = ts_tt!($other);
      build_ts_type!($stack $ty $($rest)*)
  }};
  ($stack:ident $ty:ident < $arg:tt $($rest:tt)*) => {{
      $ty = $ty.as_generic(vec![]);
      $stack.push($ty);
      let mut arg = ts_tt!($arg);
      build_ts_type!($stack arg $($rest)*)
  }};
  ($stack:ident $ty:ident , $next:tt $($rest:tt)*) => {{
      let top: crate::ts_type::TsType = $stack.pop().unwrap_or_else(|| {
          panic!("Unexpected `,` found.");
      });
      $stack.push(top.join($ty).unwrap());
      let mut next = ts_tt!($next);
      build_ts_type!($stack next $($rest)*)
  }};
  ($stack:ident $arg:ident > $($rest:tt)*) => {{
      let mut ty = $arg;
      loop {
          let top: crate::ts_type::TsType = $stack.pop().unwrap_or_else(|| { panic!("Unmatched `>` found."); });
          ty = top.join(ty).unwrap();
          if let crate::ts_type::TsType::Generic(_, _) = ty { break; }
          else if $stack.is_empty() { panic!("Unmatched `>` found."); }
      }
      build_ts_type!($stack ty $($rest)*)
  }};
  ($stack:ident $arg:ident >> $($rest:tt)*) => {{
      let mut ty = $arg;
      let mut count = 0;
      loop {
          let top: crate::ts_type::TsType = $stack.pop().unwrap_or_else(|| { panic!("Unmatched `>` found."); });
          ty = top.join(ty).unwrap();
          if let crate::ts_type::TsType::Generic(_, _) = ty { count += 1; }
          if count == 2 { break; }
          else if $stack.is_empty() { panic!("Unmatched `>` found."); }
      }
      build_ts_type!($stack ty $($rest)*)
  }};
  ($stack:ident $ty:ident) => {{
      let mut ty = $ty;
      for _ in 0..$stack.len() {
          let top: crate::ts_type::TsType = $stack.pop().unwrap();
          ty = top.join(ty).unwrap();
      }
      ty
  }};
}

macro_rules! ts_type {
  (| $member:tt $($rest:tt)*) => {{
      let _union = crate::ts_type::TsType::Union(vec![]);
      let mut stack = vec![_union];
      let mut member = ts_tt!($member);
      build_ts_type!(stack member $($rest)*)
  }};
  ($elem:tt [] $($rest:tt)*) => {{
      let mut stack = vec![];
      let mut array = crate::ts_type::TsType::Array(Box::new(ts_tt!($elem)));
      build_ts_type!(stack array $($rest)*)
  }};
  ($object:tt [ $($key:tt)+ ] $($rest:tt)*) => {{
      let mut stack = vec![];
      let object = crate::ts_type::TsType::IndexedAccess(Box::new(ts_tt!($object)), Box::new(ts_type!($($key)+)));
      build_ts_type!(stack object $($rest)*)
  }};
  ($generic:tt < $arg:tt $($rest:tt)*) => {{
      let generic = crate::ts_type::TsType::Generic(Box::new(ts_tt!($generic)), vec![]);
      let mut stack = vec![generic];
      let mut arg = ts_tt!($arg);
      build_ts_type!(stack arg $($rest)*)
  }};
  ($base:tt $($rest:tt)*) => {{
      let mut stack = vec![];
      let base = ts_tt!($base);
      build_ts_type!(stack base $($rest)*)
  }};
}

use syn::{GenericArgument, PathArguments, Type};

/// A TypeScript type.
///
/// This type is used to represent TypeScript types in a Rust-friendly way.
///
/// Note: This is not an exhaustive representation of TypeScript types.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum TsType {
    /// A single type, e.g., `string`.
    Base(String),
    Paren(Box<TsType>),
    Array(Box<TsType>),
    Tuple(Vec<TsType>),
    Union(Vec<TsType>),
    Intersection(Vec<TsType>),
    /// A generic type with arguments, e.g., `Set<string, number>`.
    Generic(Box<TsType>, Vec<TsType>),
    /// A type with an indexing key type, e.g., `Car["make"]`.
    IndexedAccess(Box<TsType>, Box<TsType>),
}

impl Default for TsType {
    fn default() -> Self {
        TsType::Base("any".to_string())
    }
}

// Error //

pub struct TsTypeError {
    pub message: String,
    pub location: String,
}

impl fmt::Display for TsTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TypeError: {}\n    Location: {}",
            self.message, self.location
        )
    }
}

impl fmt::Debug for TsTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TypeError: {}\n    Location: {}",
            self.message, self.location
        )
    }
}

// Methods //

#[allow(dead_code, clippy::wrong_self_convention)]
impl TsType {
    /// Find out if this type is a union with another type
    pub fn is_union_with(&self, other: &Self) -> bool {
        match self {
            Self::Union(types) => types.iter().any(|ty| ty == other),
            _ => false,
        }
    }

    /// Find out if this type contains another type e.g. `(string | number)[]`
    /// contains `string`
    pub fn contains(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        match self {
            Self::Base(name) => match other {
                // compare the names of 2 base types
                Self::Base(other_name) => name == other_name,
                // a base type can't contain anything other than itself
                _ => false,
            },
            Self::Paren(inner) => inner.contains(other),
            Self::Array(inner) => inner.contains(other),
            Self::Tuple(types) => types.iter().any(|ty| ty.contains(other)),
            Self::Union(types) => types.iter().any(|ty| ty.contains(other)),
            Self::Intersection(types) => types.iter().any(|ty| ty.contains(other)),
            Self::Generic(base, args) => {
                base.contains(other) || args.iter().any(|arg| arg.contains(other))
            }
            Self::IndexedAccess(base, key) => base.contains(other) || key.contains(other),
        }
    }

    /// convert this type to a generic with arguments
    pub fn as_generic(self, args: Vec<Self>) -> Self {
        match self {
            Self::Base(_) => Self::Generic(Box::new(self), args),
            Self::IndexedAccess(_, _) => Self::Generic(Box::new(self), args),
            _ => panic!("Type can't be generic."),
        }
    }

    /// convert this type to an indexed access type with a key
    pub fn property(self, key: Self) -> Self {
        Self::IndexedAccess(Box::new(self), Box::new(key))
    }

    /// place this type in an array
    pub fn in_array(self) -> Self {
        Self::Array(Box::new(self))
    }

    /// place this type in parentheses
    pub fn in_parens(self) -> Self {
        match self {
            Self::Intersection(_) => Self::Paren(Box::new(self)),
            Self::Union(_) => Self::Paren(Box::new(self)),
            // parens have no effect on base types
            _ => self,
        }
    }

    /// union this type with another
    pub fn or(self, other: Self) -> Self {
        match self {
            Self::Union(mut types) => match other {
                // a | b or c | d -> a | b | c | d
                Self::Union(mut other_types) => {
                    types.append(&mut other_types);
                    Self::Union(types)
                }
                // a | b or _ => a | b | _
                _ => {
                    types.push(other);
                    Self::Union(types)
                }
            },
            _ => match other {
                // _ or a | b -> _ | a | b
                Self::Union(mut other_types) => {
                    other_types.insert(0, self);
                    Self::Union(other_types)
                }
                // _ or _ -> _ | _
                _ => Self::Union(vec![self, other]),
            },
        }
    }

    /// intersect this type with another
    pub fn and(self, other: Self) -> Self {
        match self {
            Self::Intersection(mut types) => match other {
                // a & b and c & d -> a & b & c & d
                Self::Intersection(mut other_types) => {
                    types.append(&mut other_types);
                    Self::Intersection(types)
                }
                // a & b and _ -> a & b & _
                _ => {
                    types.push(other);
                    Self::Intersection(types)
                }
            },
            _ => match other {
                // _ and a & b -> _ & a & b
                Self::Intersection(mut other_types) => {
                    other_types.insert(0, self);
                    Self::Intersection(other_types)
                }
                // _ and _ -> _ & _
                _ => Self::Intersection(vec![self, other]),
            },
        }
    }

    /// Attempts to join this type with a right hand type.
    /// This is used during parsing and may not be intuitive for general use.
    pub fn join(self, other: Self) -> Result<Self, &'static str> {
        match self {
            Self::Base(mut str) => match other {
                // a join b -> ab
                Self::Base(other_str) => {
                    str.push_str(&other_str);
                    Ok(Self::Base(str))
                }
                // a join _ => _ join a
                _ => other.join(Self::Base(str)),
            },
            // a<b> join c -> a<b, c>
            Self::Generic(ty, mut args) => {
                args.push(other);
                Ok(Self::Generic(ty, args))
            }
            // a[b | c] join d -> a[b | c | d]
            Self::IndexedAccess(object, key) => {
                let key_inner = *key;
                Ok(Self::IndexedAccess(
                    object,
                    Box::new(key_inner.join(other)?),
                ))
            }
            Self::Union(mut types) => match other {
                // a | b join c | d -> a | b | c | d
                Self::Union(mut other_types) => {
                    types.append(&mut other_types);
                    Ok(Self::Union(types))
                }
                // a | b join _ => a | b | _
                _ => {
                    types.push(other);
                    Ok(Self::Union(types))
                }
            },
            Self::Intersection(mut types) => match other {
                // a & b join c & d -> a & b & c & d
                Self::Intersection(mut other_types) => {
                    types.append(&mut other_types);
                    Ok(Self::Intersection(types))
                }
                // a & b join c | d -> (a & b & c) | d
                Self::Union(mut union_types) => {
                    let first_member = union_types.remove(0);
                    let intersection = Self::Intersection(types);
                    let intersected_member = intersection.and(first_member);
                    union_types.insert(0, intersected_member);
                    Ok(Self::Union(union_types))
                }
                // a & b join _ -> a & b & _
                _ => {
                    types.push(other);
                    Ok(Self::Intersection(types))
                }
            },
            // [a, b] join c -> [a, b, c]
            Self::Tuple(mut types) => {
                types.push(other);
                Ok(Self::Tuple(types))
            }
            // (a | b) join c -> (a | b | c)
            Self::Paren(inner) => inner.join(other),
            _ => Err("Type does not support joining."),
        }
    }

    /// Attempts to parse a TypeScript type from a typescript string.
    ///
    /// Returns a [`TsTypeError`] if the string is empty or contains an unexpected character.
    #[track_caller]
    pub fn from_ts_str(str: &str) -> Result<Self, TsTypeError> {
        let location = Location::caller();

        if str.is_empty() {
            return Err(type_error_at!(location, "Empty string."));
        }

        let mut stacks: Vec<Vec<Self>> = vec![];
        let mut pending_stack: Vec<Self> = vec![];
        let mut pending_type: Option<Self> = None;
        let mut ambiguous_bracket = false;
        let chars = str.trim().chars();

        for char in chars {
            if ambiguous_bracket && char != ']' {
                pending_stack.push(pending_type.unwrap().property(TsType::Base("".to_string())));
                pending_type = None;
                ambiguous_bracket = false;
            }
            match char {
                ' ' => continue,
                '|' => {
                    // push pending as union or an empty union since types can
                    // start with `|`
                    let member = match pending_type {
                        Some(ty) => vec![ty],
                        None => vec![],
                    };
                    let _union = Self::Union(member);
                    pending_stack.push(_union);
                    pending_type = None;
                }
                '&' => {
                    // push pending as union or an empty union since types can
                    // start with `&`
                    let member = match pending_type {
                        Some(ty) => vec![ty],
                        None => vec![],
                    };
                    let intersection = Self::Intersection(member);
                    pending_stack.push(intersection);
                    pending_type = None;
                }
                '<' => {
                    // push pending as generic
                    if pending_type.is_none() {
                        return Err(type_error_at!(location, "Unexpected `<` found."));
                    }
                    let inner = pending_type.unwrap();
                    let generic = inner.as_generic(vec![]);
                    pending_stack.push(generic);
                    pending_type = None;
                }
                ',' => {
                    // collapse the stack to the nearest comma delimited type,
                    // joining the pending type with the top of the stack
                    if pending_type.is_none() {
                        return Err(type_error_at!(location, "Unexpected `,` found."));
                    }
                    let mut inner = pending_type.unwrap();

                    loop {
                        let top = pending_stack.pop().unwrap();
                        inner = top.join(inner).unwrap();

                        match inner {
                            Self::Generic(_, _) => break,
                            Self::IndexedAccess(_, _) => break,
                            Self::Tuple(_) => break,
                            _ => {}
                        }

                        if pending_stack.is_empty() {
                            return Err(type_error_at!(location, "Unexpected `,` found."));
                        }
                    }
                    pending_stack.push(inner);
                    pending_type = None;
                }
                '>' => {
                    // collapse the stack to the nearest generic, joining the
                    // pending type with the top of the stack
                    if pending_type.is_none() {
                        return Err(type_error_at!(location, "Unexpected `>` found."));
                    };
                    let mut ty = pending_type.unwrap();
                    loop {
                        let top = pending_stack.pop().unwrap();
                        ty = top.join(ty).unwrap();

                        if let Self::Generic(_, _) = ty {
                            break;
                        }

                        if pending_stack.is_empty() {
                            return Err(type_error_at!(location, "Unexpected `,` found."));
                        }
                    }
                    pending_type = Some(ty);
                }
                '[' => {
                    if pending_type.is_none() {
                        // If there's no pending type, open a tuple
                        let tuple = Self::Tuple(vec![]);
                        pending_stack.push(tuple);
                    } else {
                        // Otherwise, we'll have to wait to see if it's the
                        // start of an array or an indexed access type
                        ambiguous_bracket = true;
                    }
                }
                ']' => {
                    if pending_type.is_none() {
                        return Err(type_error_at!(location, "Unexpected `]` found."));
                    };
                    let mut ty = pending_type.unwrap();

                    // If there's an ambiguous bracket, it's an array, otherwise
                    // it's the end of a bracketed type on the stack.
                    if ambiguous_bracket {
                        pending_type = Some(ty.in_array());
                        ambiguous_bracket = false;
                    } else {
                        loop {
                            let top = pending_stack.pop().unwrap();
                            ty = top.join(ty).unwrap();

                            match ty {
                                Self::IndexedAccess(_, _) => break,
                                Self::Tuple(_) => break,
                                _ => {}
                            }

                            if pending_stack.is_empty() {
                                return Err(type_error_at!(location, "Unexpected `]` found."));
                            }
                        }
                        pending_type = Some(ty);
                    }
                }
                '(' => {
                    if pending_type.is_some() {
                        return Err(type_error_at!(location, "Unexpected `(` found."));
                    };

                    // push the pending stack and start a new one for the nested type
                    stacks.push(pending_stack);
                    pending_stack = vec![];
                }
                ')' => {
                    // collapse all pending types and pop the stack to exit the
                    // nested type
                    if pending_type.is_none() {
                        return Err(type_error_at!(location, "Unexpected `)` found."));
                    };
                    let mut inner = pending_type.unwrap();

                    for _ in 0..pending_stack.len() {
                        let top = pending_stack.pop().unwrap();
                        inner = top.join(inner).unwrap();
                    }

                    pending_type = Some(inner.in_parens());
                    pending_stack = stacks.pop().unwrap();
                }
                part => match pending_type {
                    Some(pending) => {
                        // If there's a pending type, join it with the next part
                        let next = Self::Base(part.to_string());
                        pending_type = Some(pending.join(next).unwrap());
                    }
                    None => {
                        // Otherwise, start a new pending type
                        pending_type = Some(Self::Base(part.to_string()));
                    }
                },
            }
        }

        let mut final_ty = pending_type.unwrap_or_else(|| pending_stack.pop().unwrap());

        while let Some(top) = pending_stack.pop() {
            final_ty = top.join(final_ty).unwrap();
        }

        Ok(final_ty)
    }
}

// Formatting //

impl fmt::Display for TsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsType::Base(name) => write!(f, "{}", name.trim()),
            TsType::Array(ty) => match ty.as_ref() {
                // Wrap the inner type in parentheses if it's a union or intersection
                // a | b[] -> (a | b)[]
                TsType::Union(_) => write!(f, "({})[]", ty),
                TsType::Intersection(_) => write!(f, "({})[]", ty),
                _ => write!(f, "{}[]", ty),
            },
            TsType::Paren(ty) => write!(f, "({})", ty),
            TsType::IndexedAccess(ty, key_ty) => {
                write!(f, "{}[{}]", ty, key_ty)
            }
            TsType::Generic(name, args) => {
                let args = args
                    .iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{}<{}>", name, args)
            }
            TsType::Union(types) => {
                let types = types
                    .iter()
                    .map(|ty| match ty {
                        // Wrap the inner type in parentheses if it's a intersection.
                        // a & b | c -> (a & b) | c
                        TsType::Intersection(_) => format!("({})", ty),
                        _ => ty.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                write!(f, "{}", types)
            }
            TsType::Intersection(types) => {
                let types = types
                    .iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<_>>()
                    .join(" & ");
                write!(f, "{}", types)
            }
            TsType::Tuple(types) => {
                let types = types
                    .iter()
                    .map(|ty| ty.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "[{}]", types)
            }
        }
    }
}

impl fmt::Debug for TsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsType::Base(name) => write!(f, "Base({})", name.trim()),
            TsType::Array(ty) => write!(f, "Array({:?})", ty),
            TsType::Paren(ty) => write!(f, "Paren({:?})", ty),
            TsType::IndexedAccess(ty, key_ty) => {
                write!(f, "IndexedAccess({:?}[{:?}])", ty, key_ty)
            }
            TsType::Generic(name, args) => {
                write!(f, "Generic(")?;
                write!(f, "{:?}<", name)?;

                for (i, arg) in args.iter().enumerate() {
                    write!(f, "{:?}", arg)?;
                    if i < args.len() - 1 {
                        write!(f, ", ")?;
                    }
                }

                write!(f, ">)")
            }
            TsType::Union(types) => {
                write!(f, "Union(")?;

                for (i, ty) in types.iter().enumerate() {
                    write!(f, "{:?}", ty)?;
                    if i < types.len() - 1 {
                        write!(f, " | ")?;
                    }
                }

                write!(f, ")")
            }
            TsType::Intersection(types) => {
                write!(f, "Intersection(")?;

                for (i, ty) in types.iter().enumerate() {
                    write!(f, "{:?}", ty)?;
                    if i < types.len() - 1 {
                        write!(f, " & ")?;
                    }
                }

                write!(f, ")")
            }
            TsType::Tuple(types) => {
                write!(f, "Tuple([")?;

                for (i, ty) in types.iter().enumerate() {
                    write!(f, "{:?}", ty)?;
                    if i < types.len() - 1 {
                        write!(f, ", ")?;
                    }
                }

                write!(f, "])")
            }
        }
    }
}

// Conversions //

impl TryFrom<&Type> for TsType {
    type Error = TsTypeError;

    #[track_caller]
    fn try_from(ty: &Type) -> Result<Self, Self::Error> {
        let location = Location::caller();

        match ty {
            Type::Path(type_path) => {
                let segment =
                    type_path.path.segments.last().ok_or_else(|| {
                        type_error_at!(location, "No segments found in type path.")
                    })?;
                let ident = &segment.ident;
                let ident_str = ident.to_string();

                match ident_str.as_str() {
                    "Option" => {
                        let PathArguments::AngleBracketed(args) = &segment.arguments else {
                            return Err(type_error_at!(
                                location,
                                "Expected type argument for Option"
                            ));
                        };
                        let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
                            return Err(type_error_at!(
                                location,
                                "Expected type argument for Option"
                            ));
                        };
                        let inner_ts = TsType::try_from(inner_ty)?;
                        Ok(inner_ts.or(ts_type!(undefined)))
                    }
                    "Vec" => {
                        let PathArguments::AngleBracketed(args) = &segment.arguments else {
                            return Err(type_error_at!(location, "Expected type argument for Vec"));
                        };
                        let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
                            return Err(type_error_at!(location, "Expected type argument for Vec"));
                        };

                        if let Type::Path(inner_path) = inner_ty
                            && let Some(inner_segment) = inner_path.path.segments.last()
                            && let Some(typed_array) =
                                match_typed_array(&inner_segment.ident.to_string())
                        {
                            Ok(typed_array)
                        } else {
                            let inner_ts = TsType::try_from(inner_ty)?;
                            Ok(inner_ts.in_array())
                        }
                    }
                    "Arc" | "Rc" | "Box" => {
                        let PathArguments::AngleBracketed(args) = &segment.arguments else {
                            return Err(type_error_at!(
                                location,
                                "Expected type argument for wrapper"
                            ));
                        };
                        let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
                            return Err(type_error_at!(
                                location,
                                "Expected type argument for wrapper"
                            ));
                        };
                        TsType::try_from(inner_ty)
                    }
                    _ => {
                        if let Some(simple) = match_simple_type(&ident_str) {
                            Ok(simple)
                        } else {
                            let mut args_vec = Vec::new();
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                for arg in &args.args {
                                    if let GenericArgument::Type(inner_ty) = arg {
                                        args_vec.push(TsType::try_from(inner_ty)?);
                                    }
                                }
                            }

                            let base = TsType::Base(ident_str);
                            if args_vec.is_empty() {
                                Ok(base)
                            } else {
                                Ok(TsType::Generic(Box::new(base), args_vec))
                            }
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
                    return Ok(ts_type!(string));
                }

                // Handle slice `&[T]`
                if let Type::Slice(type_slice) = inner_ty {
                    if let Type::Path(inner_path) = &*type_slice.elem
                        && let Some(inner_segment) = inner_path.path.segments.last()
                        && let Some(typed_array) =
                            match_typed_array(&inner_segment.ident.to_string())
                    {
                        return Ok(typed_array);
                    }

                    let inner_ts = TsType::try_from(&*type_slice.elem)?;
                    return Ok(inner_ts.in_array());
                }

                // Strip reference and map
                TsType::try_from(inner_ty)
            }
            Type::ImplTrait(type_impl) => {
                // Find `Into<X>` or `AsRef<X>`
                for bound in &type_impl.bounds {
                    if let syn::TypeParamBound::Trait(trait_bound) = bound
                        && let Some(segment) = trait_bound.path.segments.last()
                        && (segment.ident == "Into" || segment.ident == "AsRef")
                        && let PathArguments::AngleBracketed(args) = &segment.arguments
                        && let Some(GenericArgument::Type(inner_ty)) = args.args.first()
                    {
                        return TsType::try_from(inner_ty);
                    }
                }
                Err(type_error_at!(
                    location,
                    "Unsupported `impl Trait`. Only `impl Into<T>` and `impl AsRef<T>` are supported."
                ))
            }
            Type::Slice(type_slice) => {
                if let Type::Path(inner_path) = &*type_slice.elem
                    && let Some(inner_segment) = inner_path.path.segments.last()
                    && let Some(typed_array) = match_typed_array(&inner_segment.ident.to_string())
                {
                    Ok(typed_array)
                } else {
                    let inner_ts = TsType::try_from(&*type_slice.elem)?;
                    Ok(inner_ts.in_array())
                }
            }
            _ => {
                let rust_type_str = strip_type(ty)?;
                TsType::from_ts_str(&rust_type_str)
            }
        }
    }
}

impl FromStr for TsType {
    type Err = TsTypeError;

    #[track_caller]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TsType::from_ts_str(s)
    }
}

pub trait ToTsType {
    fn to_ts_type(&self) -> Result<TsType, TsTypeError>;
}

impl ToTsType for Type {
    #[track_caller]
    fn to_ts_type(&self) -> Result<TsType, TsTypeError> {
        TsType::try_from(self)
    }
}

impl ToTsType for &Type {
    #[track_caller]
    fn to_ts_type(&self) -> Result<TsType, TsTypeError> {
        TsType::try_from(*self)
    }
}

impl ToTsType for &str {
    #[track_caller]
    fn to_ts_type(&self) -> Result<TsType, TsTypeError> {
        TsType::from_str(self)
    }
}

impl ToTsType for String {
    #[track_caller]
    fn to_ts_type(&self) -> Result<TsType, TsTypeError> {
        TsType::from_str(self.as_str())
    }
}

// Internal helpers //

/// Strips references, pointers, and paths to get the base type string of a Rust type.
#[track_caller]
fn strip_type(ty: &Type) -> Result<String, TsTypeError> {
    let location = Location::caller();
    match ty {
        Type::Group(group) => strip_type(&group.elem),
        Type::Paren(paren) => strip_type(&paren.elem),
        Type::Ptr(ptr) => strip_type(&ptr.elem),
        Type::Reference(reference) => strip_type(&reference.elem),
        Type::Slice(type_slice) => Ok(format!("{}[]", strip_type(&type_slice.elem)?)),
        Type::Array(type_array) => Ok(format!("[{}; _]", strip_type(&type_array.elem)?)),
        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                Ok("()".to_string())
            } else {
                let types = tuple
                    .elems
                    .iter()
                    .map(strip_type)
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                Ok(format!("({})", types))
            }
        }
        Type::Path(path) => {
            let last_segment = path
                .path
                .segments
                .last()
                .ok_or_else(|| type_error_at!(location, "No segments found"))?;
            let outer_type = last_segment.ident.to_string();

            if last_segment.arguments.is_empty() {
                Ok(outer_type)
            } else {
                let arguments = match &last_segment.arguments {
                    PathArguments::AngleBracketed(angle) => {
                        let args = angle
                            .args
                            .iter()
                            .map(|arg| match arg {
                                GenericArgument::Type(ty) => strip_type(ty),
                                _ => Err(type_error_at!(location, "Unsupported type argument.",)),
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .join(", ");
                        format!("<{}>", args)
                    }
                    PathArguments::Parenthesized(paren) => {
                        let inputs = paren
                            .inputs
                            .iter()
                            .map(strip_type)
                            .collect::<Result<Vec<_>, _>>()?
                            .join(", ");
                        format!("({})", inputs)
                    }
                    _ => String::new(),
                };

                Ok(format!("{}{}", last_segment.ident, arguments))
            }
        }

        _ => Err(type_error_at!(location, "Unsupported type.")),
    }
}

/// Matches a "simple" Rust type to a TypeScript type.
fn match_simple_type(rust_type: &str) -> Option<TsType> {
    let simple_match = match rust_type {
        // native types
        "bool" => ts_type!(boolean),
        "String" | "str" | "char" => ts_type!(string),
        "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "f32" | "f64" => ts_type!(number),
        "u64" | "i64" | "u128" | "i128" | "usize" | "isize" => ts_type!(bigint),

        // ethers types
        "U256" | "I256" => ts_type!(bigint),
        "Address" => TsType::from_ts_str("`0x${string}`").unwrap(),

        // js_sys types
        "BigInt" => ts_type!(bigint),
        "Boolean" => ts_type!(boolean),
        "JsString" => ts_type!(string),
        "Number" => ts_type!(number),
        "Object" => ts_type!(object),
        "JsValue" => ts_type!(any),

        // other known types
        "FixedPoint" => ts_type!(bigint),

        // No simple match found
        _ => return None,
    };
    Some(simple_match)
}

fn match_typed_array(rust_type: &str) -> Option<TsType> {
    let array_name = match rust_type {
        "u8" => "Uint8Array",
        "i8" => "Int8Array",
        "u16" => "Uint16Array",
        "i16" => "Int16Array",
        "u32" => "Uint32Array",
        "i32" => "Int32Array",
        "f32" => "Float32Array",
        "f64" => "Float64Array",
        "u64" => "BigUint64Array",
        "i64" => "BigInt64Array",
        _ => return None,
    };
    Some(TsType::Base(array_name.to_string()))
}

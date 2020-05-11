//! Standard and custom namespaces.
//!
//! This module provides:
//! * the [`Namespace`](struct.Namespace.html) type for defining custom namespace;
//! * modules corresponding to the most common namespaces.
//!
//! # Example
//! ```
//! use sophia_term::ns::{Namespace, rdf, rdfs, xsd};
//!
//! let schema = Namespace::new("http://schema.org/").unwrap();
//! let s_name = schema.get("name").unwrap();
//! // and then, given a graph:
//! //g.insert(&s_name, &rdf::type_, &rdf::Property);
//! //g.insert(&s_name, &rdfs::range, &xsd::string);
//! ```

use crate::{iri::Iri, Result, Term, TermData, TermError};
use sophia_iri::is_valid_iri_ref;

/// A custom namespace.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Namespace<T: TermData>(pub(crate) T);

impl<T: TermData> Namespace<T> {
    /// Build a custom namespace based on the given IRI.
    ///
    /// `iri` must be a valid IRI, otherwise this constructor returns an error.
    pub fn new(iri: T) -> Result<Namespace<T>> {
        if is_valid_iri_ref(iri.as_ref()) {
            Ok(Namespace(iri))
        } else {
            Err(TermError::InvalidIri(iri.as_ref().to_string()))
        }
    }

    /// Build an IRI term by appending `suffix` to this namespace.
    ///
    /// Return an error if the concatenation produces an invalid IRI.
    ///
    /// Internally this method calls [`get_iri()`](#method.get_iri).
    pub fn get<U>(&self, suffix: U) -> Result<Term<T>>
    where
        U: AsRef<str>,
        T: From<U>,
    {
        self.get_iri(suffix).map(Into::into)
    }

    /// Build an IRI by appending `suffix` to this namespace.
    ///
    /// Return an error if the concatenation produces an invalid IRI.
    pub fn get_iri<U>(&self, suffix: U) -> Result<Iri<T>>
    where
        U: AsRef<str>,
        T: From<U>,
    {
        Iri::new_suffixed(self.0.clone(), suffix)
    }

    /// Maps this Namespace to another one by applying function `f`.
    pub fn map<TD2, F>(self, f: F) -> Namespace<TD2>
    where
        TD2: TermData,
        F: FnOnce(T) -> TD2,
    {
        Namespace(f(self.0))
    }

    /// Tries to map this Namespace to another one by applying function `f`.
    pub fn try_map<TD2, F, E>(self, f: F) -> Result<Namespace<TD2>, E>
    where
        TD2: TermData,
        F: FnOnce(T) -> Result<TD2, E>,
    {
        Ok(Namespace(f(self.0)?))
    }
}

impl<TD> std::convert::TryFrom<Iri<TD>> for Namespace<TD>
where
    TD: TermData,
{
    type Error = TermError;

    /// Requires that the given `Iri` has no suffix. This can be enforced with
    /// the [`clone_no_suffix()`](../iri/struct.Iri.html#method.clone_no_suffix)
    /// method.
    fn try_from(iri: Iri<TD>) -> Result<Self, Self::Error> {
        if iri.suffix().is_some() {
            Err(TermError::IsSuffixed)
        } else {
            Ok(Namespace(iri.ns))
        }
    }
}

impl<TD: TermData> AsRef<str> for Namespace<TD> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<TD: TermData> std::ops::Deref for Namespace<TD> {
    type Target = str;

    fn deref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Helper for creating a "namespace module"
/// defining a set of terms within a given IRI space.
///
/// # Safety
/// This macro is conceptually unsafe,
/// as it is never checked that the prefix IRI is a valid IRI reference.
#[macro_export]
macro_rules! namespace {
    ($iri_prefix:expr, $($suffix:ident),*; $($r_id:ident, $r_sf:expr),*) => {
        /// Prefix used in this namespace.
        pub static PREFIX:&'static str = $iri_prefix;
        $(
            $crate::ns_term!($iri_prefix, $suffix);
        )*
        $(
            $crate::ns_term!($iri_prefix, $r_id, $r_sf);
        )*

        /// Version of the terms in this namespace as `Iri`s.
        pub mod iri {
            $(
                $crate::ns_iri!($iri_prefix, $suffix);
            )*
            $(
                $crate::ns_iri!($iri_prefix, $r_id, $r_sf);
            )*
        }
    };
    ($iri_prefix:expr, $($suffix:ident),*) => {
        /// Prefix used in this namespace.
        pub static PREFIX:&'static str = $iri_prefix;
        $(
            $crate::ns_term!($iri_prefix, $suffix);
        )*

        /// Version of the terms in this namespace as `Iri`s.
        pub mod iri {
            $(
                $crate::ns_iri!($iri_prefix, $suffix);
            )*
        }
    };
}

/// Helper for creating a term in a "namespace module".
/// In general, you should use the [`namespace!`](macro.namespace.html) macro instead.
///
/// # Safety
/// This macro is conceptually unsafe,
/// as it is never checked that the prefix IRI is a valid IRI reference.
#[macro_export]
macro_rules! ns_term {
    ($prefix:expr, $ident:ident) => {
        $crate::ns_term!($prefix, $ident, stringify!($ident));
    };
    ($prefix:expr, $ident:ident, $suffix:expr) => {
        /// Generated term.
        #[allow(non_upper_case_globals)]
        pub static $ident: $crate::StaticTerm = $crate::Term::Iri(
            $crate::iri::Iri::from_raw_parts_unchecked($prefix, Some($suffix)),
        );
    };
}

/// Helper for creating a term in a "namespace module".
/// In general, you should use the [`namespace!`](macro.namespace.html) macro instead.
///
/// # Safety
/// This macro is conceptually unsafe,
/// as it is never checked that the prefix IRI is a valid IRI reference.
#[macro_export]
macro_rules! ns_iri {
    ($prefix:expr, $ident:ident) => {
        $crate::ns_iri!($prefix, $ident, stringify!($ident));
    };
    ($prefix:expr, $ident:ident, $suffix:expr) => {
        /// Generated IRI.
        #[allow(non_upper_case_globals)]
        pub static $ident: $crate::iri::Iri<&'static str> =
            $crate::iri::Iri::from_raw_parts_unchecked($prefix, Some($suffix));
    };
}

//pub static $ident:term::Term<'static> = term::Term::Iri(term::IriData{ns:$prefix, suffix:$suffix});

/// The standard `rdf:` namespace.
///
/// NB: since `type` is a reserved keyword in Rust,
/// the term `rdf:type` spells `rdf::type_` (with a trailing underscore).
///
pub mod rdf {
    namespace!(
        "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        // classes
        Alt,
        Bag,
        List,
        PlainLiteral,
        Property,
        Seq,
        Statement,
        // datatypes
        HTML,
        JSON,
        langString,
        XMLLiteral,
        // properties
        direction,
        first,
        language,
        object,
        predicate,
        rest,
        subject,
        value,
        // individuals
        nil,
        // core syntax terms
        RDF,
        ID,
        Description,
        about,
        parseType,
        resource,
        li,
        nodeID,
        datatype,
        bagID,
        aboutEach,
        aboutEachPrefix;
        // 'type' is a Rust keyword, so we use 'type_' instead
        type_, "type"
    );
}

/// The standard `xsd:` namespace.
#[rustfmt::skip]
pub mod xsd {
    namespace!(
        "http://www.w3.org/2001/XMLSchema#",
        anyType,
        anySimpleType,
            duration,
            dateTime,
            time,
            date,
            gYearMonth,
            gYear,
            gMonthDay,
            gDay,
            gMonth,
            boolean,
            base64Binary,
            hexBinary,
            float,
            double,
            anyURI,
            QName,
            NOTATION,
            string,
                normalizedString,
                    token,
                        language,
                        Name,
                            NCName,
                                ID,
                                IDREF,
                                    IDREFS,
                                ENTITY,
                                    ENTITIES,
                        NMTOKEN,
                        NMTOKENS,
            decimal,
                integer,
                    nonPositiveInteger,
                        negativeInteger,
                    long,
                        int,
                            short,
                                byte,
                    nonNegativeInteger,
                        unsignedLong,
                            unsignedInt,
                                unsignedShort,
                                    unsignedByte,
                        positiveInteger
    );
}

/// The standard `rdfs:` namespace.
pub mod rdfs {
    namespace!(
        "http://www.w3.org/2000/01/rdf-schema#",
        // types
        Class,
        Container,
        ContainerMembershipProperty,
        Datatype,
        Literal,
        Resource,
        // semantic properties
        domain,
        range,
        subClassOf,
        subPropertyOf,
        // documentation properties
        comment,
        isDefinedBy,
        label,
        member,
        seeAlso
    );
}

/// The standard `xml:` namespace
pub mod xml {
    namespace!(
        "http://www.w3.org/XML/1998/namespace#",
        lang,
        space,
        base,
        id,
        // Jon Bosak
        Father
    );
}

/// The standard `owl:` namespace
pub mod owl {
    namespace!(
        "http://www.w3.org/2002/07/owl#",
        Nothing,
        Thing,
        // Classes
        AllDifferent,
        AllDisjointClasses,
        AnnotationProperty,
        Class,
        DatatypeProperty,
        FunctionalProperty,
        InverseFunctionalProperty,
        IrreflexiveProperty,
        ObjectProperty,
        SymmetricProperty,
        TransitiveProperty,
        // Properties
        allValuesFrom,
        assertionProperty,
        complementOf,
        differentFrom,
        disjointWith,
        distinctMembers,
        equivalentClass,
        equivalentProperty,
        intersectionOf,
        inverseOf,
        maxCardinality,
        maxQualifiedCardinality,
        members,
        onClass,
        oneOf,
        onProperty,
        propertyChainAxiom,
        propertyDisjointWith,
        sameAs,
        someValuesFrom,
        sourceIndividual,
        targetIndividual,
        targetValue,
        unionOf
    );
}

#[cfg(test)]
mod test {
    // Nothing really worth testing here
    use super::*;
    use std::rc::Rc;

    #[test]
    fn test_same_term() {
        let ns1 = Namespace::new("http://schema.org/").unwrap();
        let ns2 = Namespace::new(Rc::from("http://schema.org/")).unwrap();

        assert_eq!(ns1.get("name").unwrap(), ns1.get("name").unwrap());
        assert_eq!(ns2.get("name").unwrap(), ns2.get("name").unwrap());
        assert_eq!(ns1.get("name").unwrap(), ns2.get("name").unwrap());
    }

    #[test]
    fn test_different_terms() {
        let ns1 = Namespace::new("http://schema.org/").unwrap();
        assert_ne!(ns1.get("name").unwrap(), ns1.get("nam").unwrap());
    }

    #[test]
    fn test_invalid_namespace() {
        assert!(Namespace::new("http://schema.org ").is_err());
    }

    #[test]
    fn test_invalid_suffix() {
        let ns1 = Namespace::new("http://schema.org/").unwrap();
        assert!(ns1.get("name ").is_err());
    }
}

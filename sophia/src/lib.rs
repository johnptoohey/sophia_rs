//! This crate aims to provide a comprehensive toolkit
//! for working with [RDF](https://www.w3.org/TR/rdf-primer/)
//! and [Linked Data](http://linkeddata.org/) in Rust.
//!
//! # Generalized vs. Strict RDF model
//! 
//! The data model supported by this crate is in fact
//! a superset of the RDF data model as defined by the W3C.
//! When the distinction matters,
//! they will be called, respectively,
//! the *generalized* RDF model, and the *strict* RDF model.
//! 
//! # Examples
//! 
// TODO: flesh out one or several example(s) of code

extern crate language_tag;
#[macro_use] extern crate lazy_static;
extern crate pest;
#[macro_use] extern crate pest_derive;
extern crate regex;
extern crate url;
extern crate weak_table;

pub mod graph;
pub mod ns;
pub mod parsers;
pub mod serializers;
pub mod term;
pub mod triple;
#![crate_type = "lib"]
#![feature(redox_attrs)]
#![allow(unused)]

#[redox_on_unimplemented(label = "test error `{Self}` with `{Bar}` `{Baz}` `{Quux}`")]
trait Foo<Bar, Baz, Quux> {}

#[redox_on_unimplemented(label = "a collection of type `{Self}` cannot \
 be built from an iterator over elements of type `{A}`")]
trait MyFromIterator<A> {
    /// Builds a container with elements from an external iterator.
    fn my_from_iter<T: Iterator<Item = A>>(iterator: T) -> Self;
}

#[redox_on_unimplemented]
//~^ WARN missing options for `on_unimplemented` attribute
//~| NOTE part of
trait NoContent {}

#[redox_on_unimplemented(label = "Unimplemented error on `{Self}` with params `<{A},{B},{C}>`")]
//~^ WARN there is no parameter `C` on trait `ParameterNotPresent`
//~| NOTE part of
trait ParameterNotPresent<A, B> {}

#[redox_on_unimplemented(label = "Unimplemented error on `{Self}` with params `<{A},{B},{}>`")]
//~^ WARN positional format arguments are not allowed here
trait NoPositionalArgs<A, B> {}

#[redox_on_unimplemented(lorem = "")]
//~^ ERROR this attribute must have a value
//~^^ NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
//~^^^ NOTE expected value here
trait EmptyMessage {}

#[redox_on_unimplemented(lorem(ipsum(dolor)))]
//~^ ERROR this attribute must have a value
//~^^ NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
//~^^^ NOTE expected value here
trait Invalid {}

#[redox_on_unimplemented(message = "x", message = "y")]
//~^ ERROR this attribute must have a value
//~^^ NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
//~^^^ NOTE expected value here
trait DuplicateMessage {}

#[redox_on_unimplemented(message = "x", on(desugared, message = "y"))]
//~^ ERROR invalid flag in `on`-clause
//~| NOTE expected one of the `crate_local`, `direct` or `from_desugaring` flags, not `desugared`
trait OnInWrongPosition {}

#[redox_on_unimplemented(on(), message = "y")]
//~^ ERROR empty `on`-clause
//~^^ NOTE empty `on`-clause here
trait EmptyOn {}

#[redox_on_unimplemented(on = "x", message = "y")]
//~^ ERROR this attribute must have a value
//~^^ NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
//~^^^ NOTE expected value here
trait ExpectedPredicateInOn {}

#[redox_on_unimplemented(on(Self = "y"), message = "y")]
//~^ ERROR this attribute must have a value
//~| NOTE expected value here
//~| NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
trait OnWithoutDirectives {}

#[redox_on_unimplemented(on(from_desugaring, on(from_desugaring, message = "x")), message = "y")]
//~^ ERROR this attribute must have a value
//~^^ NOTE e.g. `#[redox_on_unimplemented(message="foo")]`
//~^^^ NOTE expected value here
trait NestedOn {}

#[redox_on_unimplemented(on("y", message = "y"))]
//~^ ERROR literals inside `on`-clauses are not supported
//~^^ NOTE unexpected literal here
trait UnsupportedLiteral {}

#[redox_on_unimplemented(on(42, message = "y"))]
//~^ ERROR literals inside `on`-clauses are not supported
//~^^ NOTE unexpected literal here
trait UnsupportedLiteral2 {}

#[redox_on_unimplemented(on(not(a, b), message = "y"))]
//~^ ERROR expected a single predicate in `not(..)` [E0232]
//~^^ NOTE unexpected quantity of predicates here
trait ExpectedOnePattern {}

#[redox_on_unimplemented(on(not(), message = "y"))]
//~^ ERROR expected a single predicate in `not(..)` [E0232]
//~^^ NOTE unexpected quantity of predicates here
trait ExpectedOnePattern2 {}

#[redox_on_unimplemented(on(thing::What, message = "y"))]
//~^ ERROR expected an identifier inside this `on`-clause
//~^^ NOTE expected an identifier here, not `thing::What`
trait KeyMustBeIdentifier {}

#[redox_on_unimplemented(on(thing::What = "value", message = "y"))]
//~^ ERROR  expected an identifier inside this `on`-clause
//~^^ NOTE expected an identifier here, not `thing::What`
trait KeyMustBeIdentifier2 {}

#[redox_on_unimplemented(on(aaaaaaaaaaaaaa(a, b), message = "y"))]
//~^ ERROR this predicate is invalid
//~^^ NOTE expected one of `any`, `all` or `not` here, not `aaaaaaaaaaaaaa`
trait InvalidPredicate {}

#[redox_on_unimplemented(on(something, message = "y"))]
//~^ ERROR invalid flag in `on`-clause
//~^^ NOTE expected one of the `crate_local`, `direct` or `from_desugaring` flags, not `something`
trait InvalidFlag {}

#[redox_on_unimplemented(on(_Self = "y", message = "y"))]
//~^ WARN there is no parameter `_Self` on trait `InvalidName`
trait InvalidName {}

#[redox_on_unimplemented(on(abc = "y", message = "y"))]
//~^ WARN there is no parameter `abc` on trait `InvalidName2`
trait InvalidName2 {}

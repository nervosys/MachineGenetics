//! # std::json — JSON (First-Class)
//!
//! JSON parsing, serialization, and dynamic value manipulation.
//! JSON is first-class in MechGen because AI agents communicate via JSON.

// ---------------------------------------------------------------------------
// Serialize / Deserialize traits
// ---------------------------------------------------------------------------

/// A type that can be serialized to JSON.
pub trait Serialize {
    pub fn serialize(&self) -> Result<Value, JsonError>;
}

/// A type that can be deserialized from JSON.
pub trait Deserialize {
    pub fn deserialize(value: &Value) -> Result<Self, JsonError>;
}

// ---------------------------------------------------------------------------
// Dynamic JSON value
// ---------------------------------------------------------------------------

/// A JSON value (dynamically typed).
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    /// Check if the value is null.
    pub fn is_null(&self) -> bool {
        match self { Value::Null => true, _ => false }
    }

    /// Try to get as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self { Value::Bool(b) => Some(b), _ => None }
    }

    /// Try to get as an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self { Value::Int(n) => Some(n), _ => None }
    }

    /// Try to get as a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(f),
            Value::Int(n) => Some(n as f64),
            _ => None,
        }
    }

    /// Try to get as a string.
    pub fn as_str(&self) -> Option<&String> {
        match self { Value::Str(s) => Some(s), _ => None }
    }

    /// Try to get as an array.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self { Value::Array(a) => Some(a), _ => None }
    }

    /// Try to get as an object.
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self { Value::Object(o) => Some(o), _ => None }
    }

    /// Index into an object by key. Returns `Null` if missing.
    pub fn get(&self, key: &String) -> &Value;

    /// Index into an array by position. Returns `Null` if out of bounds.
    pub fn at(&self, idx: usize) -> &Value;

    /// Merge two objects. `other` takes precedence on conflicts.
    pub fn merge(&self, other: &Value) -> Value;

    /// Pretty-print with indentation.
    pub fn pretty(&self, indent: usize) -> String;
}

// ---------------------------------------------------------------------------
// Parse / Stringify
// ---------------------------------------------------------------------------

/// Parse a JSON string into a `Value`.
pub fn parse(input: &String) -> Result<Value, JsonError>;

/// Serialize a `Value` to a compact JSON string.
pub fn stringify(value: &Value) -> String;

/// Serialize a `Value` to a pretty-printed JSON string.
pub fn stringify_pretty(value: &Value) -> String {
    value.pretty(2)
}

/// Parse a JSON string and deserialize into type `T`.
pub fn from_str<T: Deserialize>(input: &String) -> Result<T, JsonError> {
    let value = parse(input)?;
    T::deserialize(&value)
}

/// Serialize a value of type `T` to a JSON string.
pub fn to_string<T: Serialize>(value: &T) -> Result<String, JsonError> {
    let json = value.serialize()?;
    Result::ok(stringify(&json))
}

/// Serialize a value of type `T` to a pretty-printed JSON string.
pub fn to_string_pretty<T: Serialize>(value: &T) -> Result<String, JsonError> {
    let json = value.serialize()?;
    Result::ok(stringify_pretty(&json))
}

// ---------------------------------------------------------------------------
// Standard implementations
// ---------------------------------------------------------------------------

impl Serialize for String  { pub fn serialize(&self) -> Result<Value, JsonError>; }
impl Serialize for i64     { pub fn serialize(&self) -> Result<Value, JsonError>; }
impl Serialize for f64     { pub fn serialize(&self) -> Result<Value, JsonError>; }
impl Serialize for bool    { pub fn serialize(&self) -> Result<Value, JsonError>; }
impl Serialize for Vec<T: Serialize>  { pub fn serialize(&self) -> Result<Value, JsonError>; }
impl Serialize for HashMap<String, T: Serialize> { pub fn serialize(&self) -> Result<Value, JsonError>; }

impl Deserialize for String { pub fn deserialize(v: &Value) -> Result<String, JsonError>; }
impl Deserialize for i64    { pub fn deserialize(v: &Value) -> Result<i64, JsonError>; }
impl Deserialize for f64    { pub fn deserialize(v: &Value) -> Result<f64, JsonError>; }
impl Deserialize for bool   { pub fn deserialize(v: &Value) -> Result<bool, JsonError>; }

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct JsonError {
    kind: JsonErrorKind,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
}

pub enum JsonErrorKind {
    ParseError,
    TypeError,
    MissingField,
    UnknownField,
    Other,
}

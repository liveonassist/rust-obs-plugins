#![allow(non_upper_case_globals)]
use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    marker::PhantomData,
};

use obs_sys_rs::{
    obs_data_array_count, obs_data_array_item, obs_data_array_release, obs_data_array_t,
    obs_data_clear, obs_data_create, obs_data_create_from_json, obs_data_create_from_json_file,
    obs_data_create_from_json_file_safe, obs_data_erase, obs_data_get_json, obs_data_item_byname,
    obs_data_item_get_array, obs_data_item_get_bool, obs_data_item_get_double,
    obs_data_item_get_int, obs_data_item_get_obj, obs_data_item_get_string, obs_data_item_gettype,
    obs_data_item_numtype, obs_data_item_release, obs_data_item_t, obs_data_number_type,
    obs_data_number_type_OBS_DATA_NUM_DOUBLE, obs_data_number_type_OBS_DATA_NUM_INT,
    obs_data_release, obs_data_set_default_bool, obs_data_set_default_double,
    obs_data_set_default_int, obs_data_set_default_obj, obs_data_set_default_string, obs_data_t,
    obs_data_type, obs_data_type_OBS_DATA_ARRAY, obs_data_type_OBS_DATA_BOOLEAN,
    obs_data_type_OBS_DATA_NUMBER, obs_data_type_OBS_DATA_OBJECT, obs_data_type_OBS_DATA_STRING,
    size_t,
};

use crate::{string::cstring_from_ptr, wrapper::PtrWrapper};

/// The runtime type tag of a value stored in a [`DataObj`].
///
/// Mirrors the `obs_data_type` / `obs_data_number_type` enums, collapsing
/// the integer and floating-point number sub-types into distinct
/// [`DataType::Int`] and [`DataType::Double`] variants.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum DataType {
    /// A C-string value.
    String,
    /// An integer number.
    Int,
    /// A floating-point number.
    Double,
    /// A boolean.
    Boolean,
    /// A nested key-value object — see [`DataObj`].
    Object,
    /// An ordered list of objects — see [`DataArray`].
    Array,
}

impl DataType {
    /// Computes a `DataType` from a libobs type pair.
    pub fn new(typ: obs_data_type, numtyp: obs_data_number_type) -> Self {
        match typ {
            obs_data_type_OBS_DATA_STRING => Self::String,
            obs_data_type_OBS_DATA_NUMBER => match numtyp {
                obs_data_number_type_OBS_DATA_NUM_INT => Self::Int,
                obs_data_number_type_OBS_DATA_NUM_DOUBLE => Self::Double,
                _ => unimplemented!(),
            },
            obs_data_type_OBS_DATA_BOOLEAN => Self::Boolean,
            obs_data_type_OBS_DATA_OBJECT => Self::Object,
            obs_data_type_OBS_DATA_ARRAY => Self::Array,
            _ => unimplemented!(),
        }
    }

    unsafe fn from_item(item_ptr: *mut obs_data_item_t) -> Self {
        let typ = obs_data_item_gettype(item_ptr);
        let numtyp = obs_data_item_numtype(item_ptr);
        Self::new(typ, numtyp)
    }
}

/// A type that can be read from and written to a [`DataObj`].
///
/// Implemented by the integer, floating-point, boolean, string,
/// [`DataObj`], and [`DataArray`] types so they can flow through
/// [`DataObj::get`] and [`DataObj::set_default`].
pub trait FromDataItem: Sized {
    /// Returns the [`DataType`] this implementation reads and writes.
    fn typ() -> DataType;

    /// Reads a value of this type from a libobs data item.
    ///
    /// # Safety
    ///
    /// `item` must be a valid `obs_data_item_t*` whose stored type
    /// matches [`Self::typ`].
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self>;

    /// Writes a default value for `name` into the given data object.
    ///
    /// # Safety
    ///
    /// `obj` must be a valid `obs_data_t*` and `name` must outlive the
    /// call.
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self);
}

impl FromDataItem for Cow<'_, str> {
    fn typ() -> DataType {
        DataType::String
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        let ptr = obs_data_item_get_string(item);
        if ptr.is_null() {
            return None;
        }
        Some(CStr::from_ptr(ptr).to_string_lossy())
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        let s = CString::new(val.as_ref()).unwrap();
        obs_data_set_default_string(obj, name.as_ptr(), s.as_ptr());
    }
}

impl FromDataItem for CString {
    fn typ() -> DataType {
        DataType::String
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        let ptr = obs_data_item_get_string(item);
        unsafe { cstring_from_ptr(ptr) }
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        obs_data_set_default_string(obj, name.as_ptr(), val.as_ptr());
    }
}

macro_rules! impl_get_int {
    ($($t:ty)*) => {
        $(
            impl FromDataItem for $t {
                fn typ() -> DataType {
                    DataType::Int
                }
                unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
                    Some(obs_data_item_get_int(item) as $t)
                }
                unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
                    obs_data_set_default_int(obj, name.as_ptr(), val as i64)
                }
            }
        )*
    };
}

impl_get_int!(i64 u64 i32 u32 i16 u16 i8 u8 isize usize);

impl FromDataItem for f64 {
    fn typ() -> DataType {
        DataType::Double
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        Some(obs_data_item_get_double(item))
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        obs_data_set_default_double(obj, name.as_ptr(), val)
    }
}

impl FromDataItem for f32 {
    fn typ() -> DataType {
        DataType::Double
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        Some(obs_data_item_get_double(item) as f32)
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        obs_data_set_default_double(obj, name.as_ptr(), val as f64)
    }
}

impl FromDataItem for bool {
    fn typ() -> DataType {
        DataType::Boolean
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        Some(obs_data_item_get_bool(item))
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        obs_data_set_default_bool(obj, name.as_ptr(), val)
    }
}

impl FromDataItem for DataObj<'_> {
    fn typ() -> DataType {
        DataType::Object
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        // https://github.com/obsproject/obs-studio/blob/01610d8c06edb08d0cc3155cb91b3e52e9a6473e/libobs/obs-data.c#L1798
        // `os_atomic_inc_long(&obj->ref);`
        Self::from_raw_unchecked(obs_data_item_get_obj(item))
    }
    unsafe fn set_default_unchecked(obj: *mut obs_data_t, name: &CStr, val: Self) {
        obs_data_set_default_obj(obj, name.as_ptr(), val.as_ptr_mut())
    }
}

impl FromDataItem for DataArray<'_> {
    fn typ() -> DataType {
        DataType::Array
    }
    unsafe fn from_item_unchecked(item: *mut obs_data_item_t) -> Option<Self> {
        // https://github.com/obsproject/obs-studio/blob/01610d8c06edb08d0cc3155cb91b3e52e9a6473e/libobs/obs-data.c#L1811
        // `os_atomic_inc_long(&array->ref);`
        Self::from_raw_unchecked(obs_data_item_get_array(item))
    }
    unsafe fn set_default_unchecked(_obj: *mut obs_data_t, _name: &CStr, _val: Self) {
        unimplemented!("obs_data_set_default_array function doesn't exist")
    }
}

/// A reference-counted handle to an `obs_data_t` settings object.
///
/// `DataObj` is the JSON-shaped key/value store OBS uses for plugin
/// settings. Values are accessed by name through [`DataObj::get`] and
/// [`DataObj::set_default`], which dispatch on the value's runtime type
/// via the [`FromDataItem`] trait. The contents can also be serialized
/// to and from JSON with [`DataObj::from_json`] and [`DataObj::get_json`].
///
/// The `'parent` lifetime constrains a borrowed view of a parent object
/// to its parent's lifetime; objects produced by [`DataObj::new`] or
/// [`DataObj::from_json`] are unconstrained (`'static`).
pub struct DataObj<'parent> {
    raw: *mut obs_data_t,
    _parent: PhantomData<&'parent DataObj<'parent>>,
}

impl crate::wrapper::PtrWrapperInternal for DataObj<'_> {
    unsafe fn new_internal(ptr: *mut Self::Pointer) -> Self {
        Self {
            raw: ptr,
            _parent: PhantomData,
        }
    }

    unsafe fn get_internal(&self) -> *mut Self::Pointer {
        self.raw
    }
}

// impl_ptr_wrapper!(DataObj, obs_data_t, @addref: obs_data_addref, obs_data_release);
impl_ptr_wrapper!(DataObj<'_>, obs_data_t, @identity, obs_data_release);

impl Default for DataObj<'_> {
    fn default() -> Self {
        DataObj::new()
    }
}

impl DataObj<'_> {
    /// Creates a new, empty data object.
    pub fn new() -> Self {
        unsafe {
            let raw = obs_data_create();
            Self::from_raw_unchecked(raw).expect("obs_data_create")
        }
    }

    /// Parses a data object from a JSON string.
    ///
    /// Returns `None` if the JSON is malformed.
    pub fn from_json(json_str: impl AsRef<CStr>) -> Option<Self> {
        unsafe {
            let raw = obs_data_create_from_json(json_str.as_ref().as_ptr());
            Self::from_raw_unchecked(raw)
        }
    }

    /// Loads a data object from a JSON file.
    ///
    /// If `backup_ext` is provided, OBS uses it as the suffix for a
    /// fallback file to consult when `json_file` is corrupted (matching
    /// the semantics of `obs_data_create_from_json_file_safe`).
    pub fn from_json_file(json_file: impl AsRef<CStr>, backup_ext: Option<&CStr>) -> Option<Self> {
        unsafe {
            let raw = if let Some(backup_ext) = backup_ext {
                obs_data_create_from_json_file_safe(
                    json_file.as_ref().as_ptr(),
                    backup_ext.as_ptr(),
                )
            } else {
                obs_data_create_from_json_file(json_file.as_ref().as_ptr())
            };
            Self::from_raw_unchecked(raw)
        }
    }

    /// Looks up `name` and decodes its value as `T`.
    ///
    /// Returns `None` if `name` is missing or its stored type does not
    /// match `T::typ()`. Integer types are implicitly narrowed or widened
    /// as the type system permits.
    pub fn get<T: FromDataItem>(&self, name: impl AsRef<CStr>) -> Option<T> {
        let name = name.as_ref();
        let mut item_ptr = unsafe { obs_data_item_byname(self.as_ptr() as *mut _, name.as_ptr()) };
        if item_ptr.is_null() {
            return None;
        }
        // Release it immediately since it is also referenced by this object.
        unsafe {
            obs_data_item_release(&mut item_ptr);
        }
        assert!(!item_ptr.is_null()); // We should not be the last holder

        let typ = unsafe { DataType::from_item(item_ptr) };

        if typ == T::typ() {
            unsafe { T::from_item_unchecked(item_ptr) }
        } else {
            None
        }
    }

    /// Sets the default value of `name`.
    ///
    /// The default is used when OBS reads `name` and no explicit value
    /// has been stored. Defaults survive across calls to [`clear`]
    /// (they live on a separate underlying `obs_data_t`).
    ///
    /// # Panics
    ///
    /// Defaulting a [`DataArray`] is not supported by libobs and will
    /// panic.
    ///
    /// [`clear`]: Self::clear
    pub fn set_default<T: FromDataItem>(&mut self, name: impl AsRef<CStr>, value: impl Into<T>) {
        unsafe { T::set_default_unchecked(self.as_ptr_mut(), name.as_ref(), value.into()) }
    }

    /// Serializes this object to a JSON string.
    ///
    /// Returns `None` if libobs declines to produce a JSON
    /// representation (typically only on out-of-memory).
    pub fn get_json(&self) -> Option<String> {
        unsafe {
            let ptr = obs_data_get_json(self.raw);
            Some(cstring_from_ptr(ptr)?.to_string_lossy().into_owned())
        }
    }

    /// Removes every key-value pair from this object.
    pub fn clear(&mut self) {
        unsafe {
            obs_data_clear(self.raw);
        }
    }

    /// Removes `name` from this object, if present.
    pub fn remove(&mut self, name: impl AsRef<CStr>) {
        unsafe {
            obs_data_erase(self.raw, name.as_ref().as_ptr());
        }
    }
}

/// A reference-counted handle to an `obs_data_array_t`.
///
/// `DataArray` is the ordered-collection counterpart to [`DataObj`];
/// elements are themselves [`DataObj`] values, looked up by index via
/// [`DataArray::get`].
pub struct DataArray<'parent> {
    raw: *mut obs_data_array_t,
    _parent: PhantomData<&'parent DataArray<'parent>>,
}

impl crate::wrapper::PtrWrapperInternal for DataArray<'_> {
    unsafe fn new_internal(raw: *mut Self::Pointer) -> Self {
        Self {
            raw,
            _parent: PhantomData,
        }
    }

    unsafe fn get_internal(&self) -> *mut Self::Pointer {
        self.raw
    }
}

impl_ptr_wrapper!(DataArray<'_>, obs_data_array_t, @identity, obs_data_array_release);

impl DataArray<'_> {
    /// Returns the number of elements in the array.
    pub fn len(&self) -> usize {
        unsafe { obs_data_array_count(self.raw) }
    }

    /// Returns `true` if the array has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the element at `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<DataObj<'_>> {
        // https://github.com/obsproject/obs-studio/blob/01610d8c06edb08d0cc3155cb91b3e52e9a6473e/libobs/obs-data.c#L1395
        // os_atomic_inc_long(&data->ref);
        let ptr = unsafe { obs_data_array_item(self.raw, index as size_t) };
        unsafe { DataObj::from_raw_unchecked(ptr) }
    }
}

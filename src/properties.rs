#![allow(non_upper_case_globals)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::{native_enum, string::ptr_or_null, wrapper::PtrWrapper};
use num_traits::{Bounded, Float, Num, NumCast, PrimInt, ToPrimitive, one};
use obs_sys_rs::{
    obs_combo_format, obs_combo_format_OBS_COMBO_FORMAT_FLOAT,
    obs_combo_format_OBS_COMBO_FORMAT_INT, obs_combo_format_OBS_COMBO_FORMAT_INVALID,
    obs_combo_format_OBS_COMBO_FORMAT_STRING, obs_combo_type,
    obs_combo_type_OBS_COMBO_TYPE_EDITABLE, obs_combo_type_OBS_COMBO_TYPE_INVALID,
    obs_combo_type_OBS_COMBO_TYPE_LIST, obs_editable_list_type,
    obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_FILES,
    obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_FILES_AND_URLS,
    obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_STRINGS, obs_path_type,
    obs_path_type_OBS_PATH_DIRECTORY, obs_path_type_OBS_PATH_FILE,
    obs_path_type_OBS_PATH_FILE_SAVE, obs_properties_add_bool, obs_properties_add_color,
    obs_properties_add_editable_list, obs_properties_add_float, obs_properties_add_float_slider,
    obs_properties_add_font, obs_properties_add_int, obs_properties_add_int_slider,
    obs_properties_add_list, obs_properties_add_path, obs_properties_add_text,
    obs_properties_create, obs_properties_destroy, obs_properties_t, obs_property_list_add_float,
    obs_property_list_add_int, obs_property_list_add_string, obs_property_list_insert_float,
    obs_property_list_insert_int, obs_property_list_insert_string, obs_property_list_item_disable,
    obs_property_list_item_remove, obs_property_t, obs_text_type, obs_text_type_OBS_TEXT_DEFAULT,
    obs_text_type_OBS_TEXT_MULTILINE, obs_text_type_OBS_TEXT_PASSWORD, size_t,
};

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    ops::RangeBounds,
    os::raw::c_int,
};

native_enum!(TextType, obs_text_type {
    Default => OBS_TEXT_DEFAULT,
    Password => OBS_TEXT_PASSWORD,
    Multiline => OBS_TEXT_MULTILINE,
});

native_enum!(PathType, obs_path_type {
    File => OBS_PATH_FILE,
    FileSave => OBS_PATH_FILE_SAVE,
    Directory => OBS_PATH_DIRECTORY,
});

native_enum!(ComboFormat, obs_combo_format {
    Invalid => OBS_COMBO_FORMAT_INVALID,
    Int => OBS_COMBO_FORMAT_INT,
    Float => OBS_COMBO_FORMAT_FLOAT,
    String => OBS_COMBO_FORMAT_STRING,
});

native_enum!(ComboType, obs_combo_type {
    Invalid => OBS_COMBO_TYPE_INVALID,
    Editable => OBS_COMBO_TYPE_EDITABLE,
    List => OBS_COMBO_TYPE_LIST,
});

native_enum!(EditableListType, obs_editable_list_type {
    Strings => OBS_EDITABLE_LIST_TYPE_STRINGS,
    Files => OBS_EDITABLE_LIST_TYPE_FILES,
    FilesAndUrls => OBS_EDITABLE_LIST_TYPE_FILES_AND_URLS,
});

/// A tree of properties OBS renders as the source-, output-, or
/// encoder-settings UI.
///
/// `Properties` wraps libobs's `obs_properties_t` and is constructed by
/// the optional `get_properties` callbacks — for example
/// [`GetPropertiesSource`](crate::source::traits::GetPropertiesSource).
/// Add fields with [`Properties::add`] (for boolean, numeric, text,
/// path, color, font, and editable-list types) and
/// [`Properties::add_list`] (for combo/dropdown lists).
pub struct Properties {
    pointer: *mut obs_properties_t,
}

impl_ptr_wrapper! {
    @ptr: pointer,
    Properties,
    obs_properties_t,
    @identity,
    obs_properties_destroy
}

impl Default for Properties {
    fn default() -> Self {
        Properties::new()
    }
}

impl Properties {
    /// Creates a new, empty property tree.
    pub fn new() -> Self {
        unsafe {
            let ptr = obs_properties_create();
            Self::from_raw_unchecked(ptr).expect("obs_properties_create")
        }
    }

    /// Adds a single property with key `name` and a localized
    /// `description` shown alongside the field.
    pub fn add<T: ObsProp>(&mut self, name: &CStr, description: &CStr, prop: T) -> &mut Self {
        unsafe {
            prop.add_to_props(self.pointer, name, description);
        }
        self
    }

    /// Adds a list (combo box) property and returns a [`ListProp`] handle
    /// for populating its items.
    ///
    /// When `editable` is true the user can also type free-form values;
    /// otherwise the list is restricted to the items added via
    /// [`ListProp::push`] / [`ListProp::insert`].
    pub fn add_list<T: ListType>(
        &mut self,
        name: &CStr,
        description: &CStr,
        editable: bool,
    ) -> ListProp<'_, T> {
        unsafe {
            let raw = obs_properties_add_list(
                self.pointer,
                name.as_ptr(),
                description.as_ptr(),
                if editable {
                    ComboType::Editable
                } else {
                    ComboType::List
                }
                .into(),
                T::format().into(),
            );
            ListProp::from_raw(raw).expect("obs_properties_add_list")
        }
    }
}

/// A handle for populating the items of a list (combo box) property.
///
/// Returned from [`Properties::add_list`]. The lifetime parameter ties
/// the handle to its parent [`Properties`]; the handle does not own the
/// underlying memory.
pub struct ListProp<'props, T> {
    raw: *mut obs_property_t,
    _props: PhantomData<&'props mut Properties>,
    _type: PhantomData<T>,
}

impl<T> PtrWrapper for ListProp<'_, T> {
    type Pointer = obs_property_t;

    unsafe fn from_raw_unchecked(raw: *mut Self::Pointer) -> Option<Self> {
        if raw.is_null() {
            None
        } else {
            Some(Self {
                raw,
                _props: PhantomData,
                _type: PhantomData,
            })
        }
    }

    unsafe fn as_ptr(&self) -> *const Self::Pointer {
        self.raw
    }

    unsafe fn get_ref(ptr: *mut Self::Pointer) -> *mut Self::Pointer {
        ptr
    }

    unsafe fn release(_ptr: *mut Self::Pointer) {}
}

impl<T: ListType> ListProp<'_, T> {
    /// Appends an entry with display label `name` and underlying `value`.
    pub fn push(&mut self, name: &CStr, value: T) {
        value.push_into(self.raw, name);
    }

    /// Inserts an entry at `index`, shifting later entries down.
    pub fn insert(&mut self, index: usize, name: &CStr, value: T) {
        value.insert_into(self.raw, name, index);
    }

    /// Removes the entry at `index`.
    pub fn remove(&mut self, index: usize) {
        unsafe {
            obs_property_list_item_remove(self.raw, index as size_t);
        }
    }

    /// Greys out the entry at `index` so it cannot be selected.
    pub fn disable(&mut self, index: usize, disabled: bool) {
        unsafe {
            obs_property_list_item_disable(self.raw, index as size_t, disabled);
        }
    }
}

/// A type that can be the underlying value of a [`ListProp`] entry.
///
/// Implemented for [`CString`], [`i64`], and [`f64`] — the three value
/// shapes OBS combo boxes can store.
pub trait ListType {
    /// Returns the [`ComboFormat`] this implementation produces.
    fn format() -> ComboFormat;
    /// Appends `self` as an entry on the underlying property.
    fn push_into(self, ptr: *mut obs_property_t, name: &CStr);
    /// Inserts `self` at `index` on the underlying property.
    fn insert_into(self, ptr: *mut obs_property_t, name: &CStr, index: usize);
}

impl ListType for CString {
    fn format() -> ComboFormat {
        ComboFormat::String
    }

    fn push_into(self, ptr: *mut obs_property_t, name: &CStr) {
        unsafe {
            obs_property_list_add_string(ptr, name.as_ptr(), self.as_ptr());
        }
    }

    fn insert_into(self, ptr: *mut obs_property_t, name: &CStr, index: usize) {
        unsafe {
            obs_property_list_insert_string(ptr, index as size_t, name.as_ptr(), self.as_ptr());
        }
    }
}

impl ListType for i64 {
    fn format() -> ComboFormat {
        ComboFormat::Int
    }

    fn push_into(self, ptr: *mut obs_property_t, name: &CStr) {
        unsafe {
            obs_property_list_add_int(ptr, name.as_ptr(), self);
        }
    }

    fn insert_into(self, ptr: *mut obs_property_t, name: &CStr, index: usize) {
        unsafe {
            obs_property_list_insert_int(ptr, index as size_t, name.as_ptr(), self);
        }
    }
}

impl ListType for f64 {
    fn format() -> ComboFormat {
        ComboFormat::Float
    }

    fn push_into(self, ptr: *mut obs_property_t, name: &CStr) {
        unsafe {
            obs_property_list_add_float(ptr, name.as_ptr(), self);
        }
    }

    fn insert_into(self, ptr: *mut obs_property_t, name: &CStr, index: usize) {
        unsafe {
            obs_property_list_insert_float(ptr, index as size_t, name.as_ptr(), self);
        }
    }
}

enum NumberType {
    Integer,
    Float,
}

/// A numeric property — either an integer or a floating-point field —
/// with an optional slider presentation.
///
/// Construct with [`NumberProp::new_int`] for integer fields or
/// [`NumberProp::new_float`] for floating-point fields, then refine
/// using the builder methods. Add to a [`Properties`] via
/// [`Properties::add`].
///
/// # Panics
///
/// When added to a [`Properties`], integer variants panic if `min`,
/// `max`, or `step` does not fit in a [`c_int`].
pub struct NumberProp<T> {
    min: T,
    max: T,
    step: T,
    slider: bool,
    typ: NumberType,
}

impl<T: PrimInt> NumberProp<T> {
    /// Creates a new integer property covering the full range of `T`,
    /// with a step of 1.
    pub fn new_int() -> Self {
        Self {
            min: T::min_value(),
            max: T::max_value(),
            step: one(),
            slider: false,
            typ: NumberType::Integer,
        }
    }
}

impl<T: Float> NumberProp<T> {
    /// Creates a new floating-point property covering the full range of
    /// `T`, with the given `step` size.
    pub fn new_float(step: T) -> Self {
        Self {
            min: T::min_value(),
            max: T::max_value(),
            step,
            slider: false,
            typ: NumberType::Float,
        }
    }
}

impl<T: Num + Bounded + Copy> NumberProp<T> {
    /// Sets the increment between adjacent valid values.
    pub fn with_step(mut self, step: T) -> Self {
        self.step = step;
        self
    }

    /// Sets the value range from a Rust [`RangeBounds`].
    ///
    /// Excluded bounds are converted to inclusive bounds by adding or
    /// subtracting the current step, so the result depends on the step
    /// configured at the time of this call.
    pub fn with_range<R: RangeBounds<T>>(mut self, range: R) -> Self {
        use std::ops::Bound::*;
        self.min = match range.start_bound() {
            Included(min) => *min,
            Excluded(min) => *min + self.step,
            std::ops::Bound::Unbounded => T::min_value(),
        };

        self.max = match range.end_bound() {
            Included(max) => *max,
            Excluded(max) => *max - self.step,
            std::ops::Bound::Unbounded => T::max_value(),
        };

        self
    }
    /// Renders the property as a slider rather than a spin box.
    pub fn with_slider(mut self) -> Self {
        self.slider = true;
        self
    }
}

/// A type that can be added to a [`Properties`] tree.
///
/// Each implementation describes a specific OBS field type
/// ([`BoolProp`], [`TextProp`], [`NumberProp`], [`ColorProp`],
/// [`FontProp`], [`PathProp`], [`EditableListProp`], …).
pub trait ObsProp {
    /// Adds this property to the given `obs_properties_t`.
    ///
    /// # Safety
    ///
    /// `p` must be a valid `obs_properties_t*`.
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr);
}

impl<T: ToPrimitive> ObsProp for NumberProp<T> {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        match self.typ {
            NumberType::Integer => {
                let min: c_int = NumCast::from(self.min).unwrap();
                let max: c_int = NumCast::from(self.max).unwrap();
                let step: c_int = NumCast::from(self.step).unwrap();

                if self.slider {
                    obs_properties_add_int_slider(
                        p,
                        name.as_ptr(),
                        description.as_ptr(),
                        min,
                        max,
                        step,
                    );
                } else {
                    obs_properties_add_int(p, name.as_ptr(), description.as_ptr(), min, max, step);
                }
            }
            NumberType::Float => {
                let min: f64 = NumCast::from(self.min).unwrap();
                let max: f64 = NumCast::from(self.max).unwrap();
                let step: f64 = NumCast::from(self.step).unwrap();

                if self.slider {
                    obs_properties_add_float_slider(
                        p,
                        name.as_ptr(),
                        description.as_ptr(),
                        min,
                        max,
                        step,
                    );
                } else {
                    obs_properties_add_float(
                        p,
                        name.as_ptr(),
                        description.as_ptr(),
                        min,
                        max,
                        step,
                    );
                }
            }
        }
    }
}

/// A boolean checkbox property.
pub struct BoolProp;

impl ObsProp for BoolProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_bool(p, name.as_ptr(), description.as_ptr());
    }
}

/// A text-input property — single-line, password-masked, or multiline.
pub struct TextProp {
    typ: TextType,
}

impl TextProp {
    /// Creates a text property of the given `typ`.
    pub fn new(typ: TextType) -> Self {
        Self { typ }
    }
}

impl ObsProp for TextProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_text(p, name.as_ptr(), description.as_ptr(), self.typ.into());
    }
}

/// A color-picker property.
pub struct ColorProp;

impl ObsProp for ColorProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_color(p, name.as_ptr(), description.as_ptr());
    }
}

/// A font-selection property.
///
/// The selected value is stored as a nested [`DataObj`](crate::data::DataObj)
/// with the following fields:
///
/// * `face` — typeface name (string).
/// * `style` — style name (string).
/// * `size` — size in points (integer).
/// * `flags` — bitmask of `OBS_FONT_*` flags (integer).
pub struct FontProp;

impl ObsProp for FontProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_font(p, name.as_ptr(), description.as_ptr());
    }
}

/// A file or directory picker property.
///
/// File pickers may carry a filter expression in OBS's native syntax, in
/// which file-type groups are separated by double semicolons and
/// extensions inside a group are separated by spaces. For example:
///
/// ```text
/// "Example types 1 and 2 (*.ex1 *.ex2);;Example type 3 (*.ex3)"
/// ```
pub struct PathProp {
    typ: PathType,
    filter: Option<CString>,
    default_path: Option<CString>,
}

impl PathProp {
    /// Creates a new path property of the given `typ`.
    pub fn new(typ: PathType) -> Self {
        Self {
            typ,
            filter: None,
            default_path: None,
        }
    }

    /// Sets the file-type filter for file pickers. Has no visible effect
    /// for directory pickers.
    pub fn with_filter(mut self, f: impl Into<CString>) -> Self {
        self.filter = Some(f.into());
        self
    }

    /// Sets the directory the picker opens to by default.
    pub fn with_default_path(mut self, d: impl Into<CString>) -> Self {
        self.default_path = Some(d.into());
        self
    }
}

impl ObsProp for PathProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_path(
            p,
            name.as_ptr(),
            description.as_ptr(),
            self.typ.into(),
            ptr_or_null(self.filter.as_deref()),
            ptr_or_null(self.default_path.as_deref()),
        );
    }
}

/// An editable list property (a user-managed list of strings, files, or
/// URLs).
pub struct EditableListProp {
    typ: EditableListType,
    filter: Option<CString>,
    default_path: Option<CString>,
}

impl EditableListProp {
    /// Creates a new editable-list property of the given `typ`.
    pub fn new(typ: EditableListType) -> Self {
        Self {
            typ,
            filter: None,
            default_path: None,
        }
    }

    /// Sets the file-type filter, in OBS's native filter syntax.
    pub fn with_filter(mut self, f: impl Into<CString>) -> Self {
        self.filter = Some(f.into());
        self
    }

    /// Sets the directory the picker opens to by default.
    pub fn with_default_path(mut self, d: impl Into<CString>) -> Self {
        self.default_path = Some(d.into());
        self
    }
}

impl ObsProp for EditableListProp {
    unsafe fn add_to_props(self, p: *mut obs_properties_t, name: &CStr, description: &CStr) {
        obs_properties_add_editable_list(
            p,
            name.as_ptr(),
            description.as_ptr(),
            self.typ.into(),
            ptr_or_null(self.filter.as_deref()),
            ptr_or_null(self.default_path.as_deref()),
        );
    }
}

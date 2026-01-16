//! DragStarting event handler for dragging attachments to external apps.
//!
//! When the user drags an attachment from the attachments pane, WebView2 fires
//! a DragStarting event. We intercept this to create a shell data object
//! containing the actual blob file paths, enabling drag-drop to Explorer/email/etc.
//!
//! Uses CompositeDataObject to wrap shell formats with custom MIME data so
//! internal drops can be detected via dataTransfer.getData().

use crate::keva_worker::get_data_path;
use keva_core::core::KevaCore;
use keva_core::types::Key;
use std::cell::Cell;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::LazyLock;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2DragStartingEventArgs;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, E_NOTIMPL, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IDataObject, IDataObject_Impl, IEnumFORMATETC,
    IEnumFORMATETC_Impl, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::ReleaseStgMedium;
use windows::Win32::System::Ole::{
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILClone, ILCreateFromPathW, ILFindLastID, ILFree, ILRemoveLastID, SHCreateDataObject,
};
use windows_core::Free;
use windows_core::{BOOL, HRESULT, implement, w};

/// Clipboard format for Chromium's custom MIME data wrapper.
static CHROMIUM_CUSTOM_CF: LazyLock<u16> = LazyLock::new(|| {
    let cf = unsafe { RegisterClipboardFormatW(w!("Chromium Web Custom MIME Data Format")) };
    debug_assert!(cf != 0, "Failed to register clipboard format");
    cf as u16
});

/// MIME type for internal attachment drag data.
const KEVA_ATTACHMENTS_MIME: &str = "application/x-keva-attachments";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct DragData {
    key: String,
    filenames: Vec<String>,
}

/// Creates a shell data object for file drag-drop using SHCreateDataObject.
/// This provides CF_HDROP plus all the shell formats that Explorer expects.
fn create_shell_data_object(paths: &[PathBuf]) -> windows::core::Result<IDataObject> {
    if paths.is_empty() {
        return Err(windows::core::Error::empty());
    }

    let mut full_pidls: Vec<*mut ITEMIDLIST> = Vec::new();
    for path in paths {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let pidl = unsafe { ILCreateFromPathW(windows::core::PCWSTR(wide.as_ptr())) };
        if !pidl.is_null() {
            full_pidls.push(pidl);
        }
    }

    if full_pidls.is_empty() {
        return Err(windows::core::Error::empty());
    }

    // Parent folder PIDL: clone first item and remove the filename component
    let parent_pidl = unsafe { ILClone(full_pidls[0]) };
    if parent_pidl.is_null() {
        for pidl in &full_pidls {
            unsafe { ILFree(Some(*pidl as *const _)) };
        }
        return Err(windows::core::Error::empty());
    }
    let _ = unsafe { ILRemoveLastID(Some(parent_pidl)) };

    // Child PIDLs point to the last item ID in each full PIDL (relative to parent)
    let child_pidls: Vec<*const ITEMIDLIST> = full_pidls
        .iter()
        .map(|&pidl| unsafe { ILFindLastID(pidl) as *const ITEMIDLIST })
        .collect();

    let result = unsafe {
        SHCreateDataObject(
            Some(parent_pidl as *const ITEMIDLIST),
            Some(&child_pidls),
            None,
        )
    };

    unsafe { ILFree(Some(parent_pidl as *const _)) };
    for pidl in &full_pidls {
        unsafe { ILFree(Some(*pidl as *const _)) };
    }

    result
}

/// Handles the DragStarting event from WebView2.
///
/// Returns true if we handled the drag (caller should set Handled=true),
/// false if we should let WebView2 handle it normally.
pub fn handle_drag_starting(
    args: &ICoreWebView2DragStartingEventArgs,
) -> windows::core::Result<bool> {
    let data_obj = unsafe { args.Data()? };

    let Some(drag_data) = extract_attachment_drag_data(&data_obj) else {
        return Ok(false);
    };

    let Ok(key) = Key::try_from(drag_data.key.as_str()) else {
        return Ok(false);
    };

    let data_path = get_data_path();
    let paths: Vec<PathBuf> = drag_data
        .filenames
        .iter()
        .map(|filename| KevaCore::attachment_blob_path(&data_path, &key, filename))
        .filter(|p| p.exists())
        .collect();

    if paths.is_empty() {
        return Ok(false);
    }

    let shell_obj = create_shell_data_object(&paths)?;

    // Serialize drag data for internal drop detection via Chromium custom MIME format
    let json = serde_json::to_string(&drag_data).unwrap_or_default();
    let custom_data = create_chromium_custom_data(KEVA_ATTACHMENTS_MIME, &json);

    // Wrap shell object with custom MIME data
    let data_obj = CompositeDataObject::wrap(shell_obj, *CHROMIUM_CUSTOM_CF, custom_data);

    let drop_source: IDropSource = SimpleDropSource.into();

    let mut effect = DROPEFFECT_NONE;
    let _ = unsafe { DoDragDrop(&data_obj, &drop_source, DROPEFFECT_COPY, &mut effect) };

    Ok(true)
}

/// Extracts attachment drag data from IDataObject if present.
fn extract_attachment_drag_data(data_obj: &IDataObject) -> Option<DragData> {
    let cf = *CHROMIUM_CUSTOM_CF;
    if cf == 0 {
        return None;
    }

    let formatetc = FORMATETC {
        cfFormat: cf,
        ptd: null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let mut medium = unsafe { data_obj.GetData(&formatetc) }.ok()?;

    let hglobal = unsafe { medium.u.hGlobal };
    let size = unsafe { GlobalSize(hglobal) };
    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        unsafe { ReleaseStgMedium(&mut medium) };
        return None;
    }

    let result = parse_chromium_custom_data(ptr, size);

    let _ = unsafe { GlobalUnlock(hglobal) };
    unsafe { ReleaseStgMedium(&mut medium) };

    result
}

/// Parses Chromium's custom MIME data format to extract our attachment data.
///
/// See `create_chromium_custom_data` for the binary format specification.
fn parse_chromium_custom_data(ptr: *mut std::ffi::c_void, size: usize) -> Option<DragData> {
    // Minimum size: data_len (4) + pair_count (4) = 8 bytes
    if size < 8 {
        return None;
    }

    let data = ptr as *const u8;

    unsafe {
        // Skip data_len at offset 0, read pair_count at offset 4
        let pair_count = *(data.add(4) as *const u32);
        let mut offset = 8usize;

        let mut read_string = || {
            // Need at least 4 bytes for length
            if offset + 4 > size {
                return None;
            }
            let len = *(data.add(offset) as *const u32) as usize;
            offset += 4;

            // Calculate string byte size with padding
            let byte_len = len * 2;
            let padded_len = if len.is_multiple_of(2) {
                byte_len
            } else {
                byte_len + 2
            };

            if offset + padded_len > size {
                return None;
            }

            let slice = std::slice::from_raw_parts(data.add(offset) as *const u16, len);
            let s = String::from_utf16_lossy(slice);
            offset += padded_len;
            Some(s)
        };

        for _ in 0..pair_count {
            let key = read_string()?;
            let value = read_string()?;

            if key == KEVA_ATTACHMENTS_MIME {
                return serde_json::from_str(&value).ok();
            }
        }
    }

    None
}

/// Serializes data into Chromium's custom MIME data format (pickle).
///
/// Binary structure:
/// ```text
/// Offset 0:   [u32 LE] data_len — byte length of everything after this field
/// Offset 4:   [u32 LE] pair_count — number of key-value pairs
///
/// For each pair:
///     [u32 LE] key_len — string length in UTF-16 code units (not bytes)
///     [key_len × 2 bytes] key — UTF-16LE encoded string
///     [0 or 2 bytes] padding — if key_len is odd, add 2 zero bytes
///     [u32 LE] value_len — string length in UTF-16 code units
///     [value_len × 2 bytes] value — UTF-16LE encoded string
///     [0 or 2 bytes] padding — if value_len is odd, add 2 zero bytes
/// ```
fn create_chromium_custom_data(mime_type: &str, value: &str) -> Vec<u8> {
    // Build payload first (pair_count + pairs)
    let mut payload = Vec::new();

    // Pair count: 1 entry (u32)
    payload.extend_from_slice(&1u32.to_le_bytes());

    // Write MIME type and value
    write_pickle_string(&mut payload, mime_type);
    write_pickle_string(&mut payload, value);

    // Build final data with data_len prefix
    let mut data = Vec::with_capacity(4 + payload.len());
    data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend(payload);

    data
}

fn write_pickle_string(data: &mut Vec<u8>, s: &str) {
    let utf16: Vec<u16> = s.encode_utf16().collect();
    let len = utf16.len() as u32;
    data.extend_from_slice(&len.to_le_bytes());
    for c in &utf16 {
        data.extend_from_slice(&c.to_le_bytes());
    }
    // Padding: if string length is odd, add 2 zero bytes for 4-byte alignment
    if !len.is_multiple_of(2) {
        data.extend_from_slice(&[0u8, 0u8]);
    }
}

/// Wraps a shell IDataObject to add custom MIME data for internal drop detection.
///
/// WebView2 exposes the Chromium custom MIME format back to JavaScript when the
/// drop occurs within the same WebView, allowing JS to read the data via
/// `dataTransfer.getData('application/x-keva-attachments')`.
#[implement(IDataObject)]
struct CompositeDataObject {
    inner: IDataObject,
    custom_cf: u16,
    custom_data: Vec<u8>,
}

impl CompositeDataObject {
    fn wrap(inner: IDataObject, custom_cf: u16, custom_data: Vec<u8>) -> IDataObject {
        let obj = Self {
            inner,
            custom_cf,
            custom_data,
        };
        obj.into()
    }

    fn is_custom_format(&self, pformatetc: *const FORMATETC) -> bool {
        if pformatetc.is_null() {
            return false;
        }
        let fmt = unsafe { &*pformatetc };
        fmt.cfFormat == self.custom_cf && fmt.tymed & TYMED_HGLOBAL.0 as u32 != 0
    }

    /// Creates an STGMEDIUM containing the custom MIME data.
    ///
    /// The returned STGMEDIUM has `pUnkForRelease: None`, meaning the caller
    /// owns the HGLOBAL and is responsible for freeing it (standard GetData semantics).
    fn create_custom_medium(&self) -> windows::core::Result<STGMEDIUM> {
        let mut hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, self.custom_data.len()) }?;

        let ptr = unsafe { GlobalLock(hglobal) };
        if ptr.is_null() {
            unsafe { hglobal.free() };
            return Err(windows::core::Error::empty());
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                self.custom_data.as_ptr(),
                ptr as *mut u8,
                self.custom_data.len(),
            );
            let _ = GlobalUnlock(hglobal);
        }

        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: windows::Win32::System::Com::STGMEDIUM_0 { hGlobal: hglobal },
            pUnkForRelease: std::mem::ManuallyDrop::new(None),
        })
    }
}

impl IDataObject_Impl for CompositeDataObject_Impl {
    fn GetData(&self, pformatetc: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        if self.is_custom_format(pformatetc) {
            return self.create_custom_medium();
        }
        unsafe { self.inner.GetData(pformatetc) }
    }

    fn GetDataHere(
        &self,
        pformatetc: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        unsafe { self.inner.GetDataHere(pformatetc, pmedium) }
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        if self.is_custom_format(pformatetc) {
            return S_OK;
        }
        unsafe { self.inner.QueryGetData(pformatetc) }
    }

    fn GetCanonicalFormatEtc(
        &self,
        pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        unsafe {
            self.inner
                .GetCanonicalFormatEtc(pformatectin, pformatetcout)
        }
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(windows::core::Error::from_hresult(E_NOTIMPL));
        }

        // Collect formats from inner, then append our custom format
        let inner_enum = unsafe { self.inner.EnumFormatEtc(dwdirection)? };
        let mut formats = Vec::new();

        loop {
            let mut fmt = [FORMATETC::default()];
            let mut fetched = 0u32;
            let hr = unsafe { inner_enum.Next(&mut fmt, Some(&mut fetched)) };
            if hr != S_OK || fetched == 0 {
                break;
            }
            formats.push(fmt[0]);
        }

        // Add our custom format (Chromium Web Custom MIME Data Format)
        formats.push(FORMATETC {
            cfFormat: self.custom_cf,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        });

        Ok(CompositeEnumFormatEtc::create(formats))
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: windows_core::Ref<'_, windows::Win32::System::Com::IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<windows::Win32::System::Com::IEnumSTATDATA> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }
}

/// Enumerator for FORMATETC that includes shell formats plus our custom format.
#[implement(IEnumFORMATETC)]
struct CompositeEnumFormatEtc {
    formats: Vec<FORMATETC>,
    index: Cell<usize>,
}

impl CompositeEnumFormatEtc {
    fn create(formats: Vec<FORMATETC>) -> IEnumFORMATETC {
        let obj = Self {
            formats,
            index: Cell::new(0),
        };
        obj.into()
    }
}

impl IEnumFORMATETC_Impl for CompositeEnumFormatEtc_Impl {
    fn Next(&self, celt: u32, rgelt: *mut FORMATETC, pceltfetched: *mut u32) -> HRESULT {
        let mut fetched = 0u32;
        let idx = self.index.get();

        for i in 0..celt as usize {
            if idx + i >= self.formats.len() {
                break;
            }
            unsafe { *rgelt.add(i) = self.formats[idx + i] };
            fetched += 1;
        }

        self.index.set(idx + fetched as usize);

        if !pceltfetched.is_null() {
            unsafe { *pceltfetched = fetched };
        }

        if fetched == celt { S_OK } else { S_FALSE }
    }

    fn Skip(&self, celt: u32) -> windows::core::Result<()> {
        let idx = self.index.get();
        let new_idx = idx.saturating_add(celt as usize);
        self.index.set(new_idx.min(self.formats.len()));
        // Always return Ok - we skip as many as possible, clamping at the end.
        // COM's S_FALSE ("fewer elements than requested") isn't an error condition.
        Ok(())
    }

    fn Reset(&self) -> windows::core::Result<()> {
        self.index.set(0);
        Ok(())
    }

    fn Clone(&self) -> windows::core::Result<IEnumFORMATETC> {
        let cloned = CompositeEnumFormatEtc {
            formats: self.formats.clone(),
            index: Cell::new(self.index.get()),
        };
        Ok(cloned.into())
    }
}

/// Simple IDropSource implementation.
#[implement(IDropSource)]
struct SimpleDropSource;

impl IDropSource_Impl for SimpleDropSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            DRAGDROP_S_CANCEL
        } else if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

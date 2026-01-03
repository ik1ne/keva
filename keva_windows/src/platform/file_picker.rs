//! File picker dialog using IFileOpenDialog.

use std::path::PathBuf;
use windows::core::{Result, w};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::{
    FileOpenDialog, IFileOpenDialog, IShellItem, IShellItemArray, FOS_ALLOWMULTISELECT,
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH,
};

/// Opens a multi-select file picker dialog.
/// Returns the selected file paths, or an empty vector if cancelled.
pub fn open_file_picker(parent: HWND) -> Vec<PathBuf> {
    open_file_picker_impl(parent).unwrap_or_default()
}

fn open_file_picker_impl(parent: HWND) -> Result<Vec<PathBuf>> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;

        // Configure options: multi-select, files only, must exist
        let options = dialog.GetOptions()?;
        dialog
            .SetOptions(options | FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM)?;

        dialog.SetTitle(w!("Add Attachments"))?;

        // Show dialog - returns error if cancelled
        if dialog.Show(Some(parent)).is_err() {
            return Ok(Vec::new());
        }

        let results: IShellItemArray = dialog.GetResults()?;
        let count = results.GetCount()?;

        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let item: IShellItem = results.GetItemAt(i)?;
            let path_ptr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path_str = path_ptr.to_string()?;
            paths.push(PathBuf::from(path_str));
            windows::Win32::System::Com::CoTaskMemFree(Some(path_ptr.as_ptr() as *const _));
        }

        Ok(paths)
    }
}

//! Development aid: `--snapshot <file.png>` writes a PNG of the web view once
//! the startup request has been shown. macOS debug builds only; it uses
//! WebKit's own snapshot, so no screen-recording permission is needed.
use std::path::PathBuf;

pub fn take(window: &tauri::WebviewWindow, destination: PathBuf) -> Result<(), String> {
    use block2::RcBlock;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError};
    use objc2_web_kit::WKWebView;
    window
        .with_webview(move |platform| {
            let destination = destination.clone();
            let handler = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
                // SAFETY: WebKit passes a valid image or null.
                let result = unsafe { image.as_ref() }
                    .and_then(|image| image.TIFFRepresentation())
                    .and_then(|tiff| NSBitmapImageRep::imageRepWithData(&tiff))
                    .and_then(|rep| unsafe {
                        rep.representationUsingType_properties(
                            NSBitmapImageFileType::PNG,
                            &NSDictionary::new(),
                        )
                    })
                    .ok_or("snapshot failed".to_string())
                    .and_then(|png| {
                        std::fs::write(&destination, png.to_vec()).map_err(|e| e.to_string())
                    });
                match result {
                    Ok(()) => eprintln!("snapshot written to {}", destination.display()),
                    Err(error) => eprintln!("snapshot: {error}"),
                }
            });
            // SAFETY: on macOS the platform web view is a WKWebView, and this
            // closure runs on the main thread.
            unsafe {
                let view = &*(platform.inner() as *const WKWebView);
                view.takeSnapshotWithConfiguration_completionHandler(None, &handler);
            }
        })
        .map_err(|e| e.to_string())
}

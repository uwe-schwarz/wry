use objc2::DeclaredClass;
use objc2_foundation::{NSHTTPURLResponse, NSObjectProtocol, NSString};
use objc2_web_kit::{
  WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationResponse,
  WKNavigationResponsePolicy,
};

#[cfg(target_os = "ios")]
use crate::wkwebview::ios::WKWebView::WKWebView;
#[cfg(target_os = "macos")]
use objc2_web_kit::WKWebView;

use crate::PageLoadEvent;

use super::class::wry_navigation_delegate::WryNavigationDelegate;

pub(crate) fn did_commit_navigation(
  this: &WryNavigationDelegate,
  webview: &WKWebView,
  _navigation: &WKNavigation,
) {
  unsafe {
    // Call on_load_handler
    if let Some(on_page_load) = &this.ivars().on_page_load_handler {
      on_page_load(PageLoadEvent::Started);
    }

    // Inject scripts
    let mut pending_scripts = this.ivars().pending_scripts.lock().unwrap();
    if let Some(scripts) = &*pending_scripts {
      for script in scripts {
        #[cfg(feature = "tracing")]
        let handler = block2::RcBlock::new(
          |_val: *mut objc2::runtime::AnyObject, err: *mut objc2_foundation::NSError| {
            if !err.is_null() {
              let description = (*err).localizedDescription().to_string();
              tracing::debug!("Exception during script eval: {description}");
            }
          },
        );
        #[cfg(feature = "tracing")]
        let handler = Some(&*handler);
        #[cfg(not(feature = "tracing"))]
        let handler = None;

        webview.evaluateJavaScript_completionHandler(&NSString::from_str(script), handler);
      }
      *pending_scripts = None;
    }
  }
}

pub(crate) fn did_finish_navigation(
  this: &WryNavigationDelegate,
  _webview: &WKWebView,
  _navigation: &WKNavigation,
) {
  if let Some(on_page_load) = &this.ivars().on_page_load_handler {
    on_page_load(PageLoadEvent::Finished);
  }
}

// Navigation handler
pub(crate) fn navigation_policy(
  this: &WryNavigationDelegate,
  _webview: &WKWebView,
  action: &WKNavigationAction,
  handler: &block2::Block<dyn Fn(WKNavigationActionPolicy)>,
) {
  unsafe {
    // <https://developer.apple.com/documentation/webkit/wknavigationaction/shouldperformdownload>
    // Available: macOS 11.3+, iOS 14.5+
    let can_download = action.respondsToSelector(objc2::sel!(shouldPerformDownload));
    let should_download: bool = if can_download {
      action.shouldPerformDownload()
    } else {
      false
    };
    let request = action.request();
    // The request URL is nullable: a navigation with no URL cannot be
    // evaluated against the navigation policy, so cancel it.
    let Some(url) = request.URL().and_then(|url| url.absoluteString()) else {
      (*handler).call((WKNavigationActionPolicy::Cancel,));
      return;
    };

    if should_download {
      let has_download_handler = this.ivars().has_download_handler;
      if has_download_handler {
        (*handler).call((WKNavigationActionPolicy::Download,));
      } else {
        (*handler).call((WKNavigationActionPolicy::Cancel,));
      }
    } else {
      let function = &this.ivars().navigation_policy_function;
      match function(url.to_string()) {
        true => (*handler).call((WKNavigationActionPolicy::Allow,)),
        false => (*handler).call((WKNavigationActionPolicy::Cancel,)),
      };
    }
  }
}

// Navigation handler
pub(crate) fn navigation_policy_response(
  this: &WryNavigationDelegate,
  _webview: &WKWebView,
  response: &WKNavigationResponse,
  handler: &block2::Block<dyn Fn(WKNavigationResponsePolicy)>,
) {
  unsafe {
    let can_show_mime_type = response.canShowMIMEType();
    let has_download_handler = this.ivars().has_download_handler;

    if has_download_handler && (!can_show_mime_type || is_attachment(response)) {
      (*handler).call((WKNavigationResponsePolicy::Download,));
      return;
    }

    (*handler).call((WKNavigationResponsePolicy::Allow,));
  }
}

/// Whether the response asks to be saved rather than shown (`Content-Disposition: attachment`).
/// Browsers download such responses even when they could display them, such as a PDF or a text
/// file. This is the rule of WebKit's own `ResourceResponse::isAttachment`.
///
/// Only with `WKDownload` (macOS 11.3+, iOS 14.5+): before it, a download that WebKit could
/// have displayed would not reach the download handler.
fn is_attachment(response: &WKNavigationResponse) -> bool {
  if objc2::runtime::AnyClass::get(c"WKDownload").is_none() {
    return false;
  }
  let response = unsafe { response.response() };
  let Some(response) = response.downcast_ref::<NSHTTPURLResponse>() else {
    return false;
  };
  let header = response.valueForHTTPHeaderField(&NSString::from_str("Content-Disposition"));
  header.is_some_and(|value| {
    let value = value.to_string();
    let disposition = value
      .split(';')
      .next()
      .unwrap_or_default()
      .trim_matches(|c: char| c.is_ascii_whitespace());
    disposition.eq_ignore_ascii_case("attachment")
  })
}

pub(crate) fn web_content_process_did_terminate(
  this: &WryNavigationDelegate,
  _webview: &WKWebView,
) {
  if let Some(on_web_content_process_terminate) =
    &this.ivars().on_web_content_process_terminate_handler
  {
    on_web_content_process_terminate();
  }
}

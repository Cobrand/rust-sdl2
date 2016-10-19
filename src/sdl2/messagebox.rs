use std::ffi::{CString, NulError};
use std::ptr;
use std::os::raw::{c_char,c_int};

use video::WindowRef;
use get_error;

use sys::messagebox as ll;

bitflags! {
    pub flags MessageBoxFlag: u32 {
        const MESSAGEBOX_ERROR =
            ::sys::messagebox::SDL_MessageBoxFlags::SDL_MESSAGEBOX_ERROR as u32,
        const MESSAGEBOX_WARNING =
            ::sys::messagebox::SDL_MessageBoxFlags::SDL_MESSAGEBOX_WARNING as u32,
        const MESSAGEBOX_INFORMATION =
            ::sys::messagebox::SDL_MessageBoxFlags::SDL_MESSAGEBOX_INFORMATION as u32
    }
}

bitflags! {
    pub flags MessageBoxButtonFlag: u32 {
        const MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT =
            ::sys::messagebox::SDL_MessageBoxButtonFlags::SDL_MESSAGEBOX_BUTTON_ESCAPEKEY_DEFAULT as u32,
        const MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT =
            ::sys::messagebox::SDL_MessageBoxButtonFlags::SDL_MESSAGEBOX_BUTTON_RETURNKEY_DEFAULT as u32,
        const MESSAGEBOX_BUTTON_NOTHING = 0
    }
}

#[derive(Debug)]
pub struct MessageBoxColorScheme {
    pub background:(u8,u8,u8),
    pub text:(u8,u8,u8),
    pub button_border:(u8,u8,u8),
    pub button_background:(u8,u8,u8),
    pub button_selected:(u8,u8,u8)
}

/// button_id is the integer that will be returned
/// by show_message_box. It is not sed by SDL2,
/// and should only be used to know which button has been triggered
#[derive(Debug)]
pub struct ButtonData<'a> {
    pub flags:MessageBoxButtonFlag,
    pub button_id:i32,
    pub text:&'a str
}

#[derive(Debug)]
pub enum ClickedButton<'a> {
    CloseButton,
    CustomButton(&'a ButtonData<'a>)
}

impl From<MessageBoxColorScheme> for [ll::SDL_MessageBoxColor ; 5] {
    fn from(scheme:MessageBoxColorScheme) -> [ll::SDL_MessageBoxColor ; 5] {
        fn to_message_box_color(t:(u8,u8,u8)) -> ll::SDL_MessageBoxColor {
            ll::SDL_MessageBoxColor {
                r:t.0,
                g:t.1,
                b:t.2
            }
        };
        [to_message_box_color(scheme.background),
        to_message_box_color(scheme.text),
        to_message_box_color(scheme.button_border),
        to_message_box_color(scheme.button_background),
        to_message_box_color(scheme.button_selected)]
    }
}


#[derive(Debug)]
pub enum ShowMessageError {
    InvalidTitle(NulError),
    InvalidMessage(NulError),
    /// Second argument of the tuple (i32) corresponds to the
    /// first button_id having an error
    InvalidButton(NulError,i32),
    SdlError(String),
}

/// Show a simple message box, meant to be informative only.
///
/// There is no way to know if the user clicked "Ok" or closed the message box,
/// If you want to retrieve which button was clicked and customize a bit more
/// your message box, use `show_message_box` instead.
pub fn show_simple_message_box(flags: MessageBoxFlag, title: &str,
        message: &str, window: Option<&WindowRef>)
        -> Result<(), ShowMessageError> {
    use self::ShowMessageError::*;
    let result = unsafe {
        let title = match CString::new(title) {
            Ok(s) => s,
            Err(err) => return Err(InvalidTitle(err)),
        };
        let message = match CString::new(message) {
            Ok(s) => s,
            Err(err) => return Err(InvalidMessage(err)),
        };
        ll::SDL_ShowSimpleMessageBox(
            flags.bits(),
            title.as_ptr() as *const c_char,
            message.as_ptr() as *const c_char,
            window.map_or(ptr::null_mut(), |win| win.raw())
        )
    } == 0;

    if result {
        Ok(())
    } else {
        Err(SdlError(get_error()))
    }
}

/// Show a customizable message box.
///
/// An array of buttons is required for it to work. The array can be empty,
/// but it will have no button beside the close button.
///
/// On success, it will return the `button_id` of the pressed/clicked button. If
/// the id is -1, the close button has been clicked, or the message box has been forcefully closed
/// (Alt-F4, ...)
///
/// You must not use -1 as acan also use -1 as a `button_id`, but it might be wise to choose another value to be able
/// to tell the difference between the close button and your custom button being clicked.
pub fn show_message_box<'a>(flags:MessageBoxFlag, buttons:&'a [ButtonData], title:&str,
    message:&str, window:Option<&WindowRef>, scheme:Option<MessageBoxColorScheme>)
    -> Result<ClickedButton<'a>,ShowMessageError> {
    use self::ShowMessageError::*;
    let mut button_id : c_int = 0;
    let title = match CString::new(title) {
        Ok(s) => s,
        Err(err) => return Err(InvalidTitle(err)),
    };
    let message = match CString::new(message) {
        Ok(s) => s,
        Err(err) => return Err(InvalidMessage(err)),
    };
    let button_texts : Result<Vec<_>,(_,i32)> = buttons.iter().map(|b|{
        CString::new(b.text).map_err(|e|(e,b.button_id))
    }).collect(); // Create CString for every button; and catch any CString Error
    let button_texts = match button_texts {
        Ok(b) => b,
        Err(e) => return Err(InvalidButton(e.0,e.1))
    };
    let raw_buttons : Vec<ll::SDL_MessageBoxButtonData> = 
        buttons.iter().zip(button_texts.iter()).map(|(b,b_text)|{
        ll::SDL_MessageBoxButtonData {
            flags:b.flags.bits(),
            buttonid:b.button_id as c_int,
            text:b_text.as_ptr()
        }
    }).collect();
    let result = unsafe {
        let msg_box_data = ll::SDL_MessageBoxData {
            flags:flags.bits(),
            window:window.map_or(ptr::null_mut(), |win| win.raw()),
            title: title.as_ptr() as *const c_char,
            message: message.as_ptr() as *const c_char,
            numbuttons: raw_buttons.len() as c_int,
            buttons: raw_buttons.as_ptr(),
            color_scheme: if let Some(scheme) = scheme {
                &ll::SDL_MessageBoxColorScheme {
                    colors:From::from(scheme)
                } as *const _
            } else {
                ptr::null()
            }
        };
        ll::SDL_ShowMessageBox(
            &msg_box_data as *const _,
            &mut button_id as &mut _
        )
    } == 0;
    if result {
        match button_id {
            -1 => Ok(ClickedButton::CloseButton),
            id => {
                let button = buttons.iter().find(|b| b.button_id == id);
                Ok(ClickedButton::CustomButton(button.unwrap()))
            }
        }
    } else {
        Err(SdlError(get_error()))
    }
}

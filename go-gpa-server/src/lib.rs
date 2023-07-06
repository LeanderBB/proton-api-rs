use std::ffi::{c_char, CStr, CString};

mod go;

pub struct Server(go::GoInt);

pub type Result<T> = std::result::Result<T, String>;

pub struct UserID(String);

impl AsRef<str> for UserID {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub struct AddressID(String);

impl AsRef<str> for AddressID {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Server {
    pub fn new() -> Result<Self> {
        let handle = unsafe { go::gpaServerNew() };
        if handle < 0 {
            return Err("Failed to create server".to_string());
        }
        Ok(Self(handle))
    }

    pub fn url(&self) -> Result<String> {
        unsafe {
            let host = go::gpaServerUrl(self.0);
            if host.is_null() {
                return Err("Invalid Server Instance".to_string());
            }
            Ok(go_char_ptr_to_str(host))
        }
    }

    pub fn create_user(
        &self,
        name: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<(UserID, AddressID)> {
        unsafe {
            let cname = CString::new(name.as_ref()).expect("Failed to convert to CString");
            let cpwd = CString::new(password.as_ref()).expect("Failed to convert to CString");
            let mut out_user_id = std::ptr::null_mut();
            let mut out_addr_id = std::ptr::null_mut();
            if go::gpaCreateUser(
                self.0,
                cname.as_ptr(),
                cpwd.as_ptr(),
                &mut out_user_id,
                &mut out_addr_id,
            ) < 0
            {
                return Err("Failed to create user".to_string());
            }

            Ok((
                UserID(go_char_ptr_to_str(out_user_id)),
                AddressID(go_char_ptr_to_str(out_addr_id)),
            ))
        }
    }

    pub fn set_auth_timeout(&self, duration: std::time::Duration) -> Result<()> {
        unsafe {
            if go::gpaSetAuthLife(self.0, duration.as_secs() as i64) < 0 {
                return Err("Failed to set auth timeout".to_string());
            }

            Ok(())
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            if go::gpaServerDelete(self.0) < 0 {
                panic!("Failed to close gpa test server")
            }
        }
    }
}

unsafe fn go_char_ptr_to_str(go_str: *mut c_char) -> String {
    let cstr = CStr::from_ptr(go_str);
    let str = cstr.to_string_lossy().to_string();
    go::CStrFree(go_str);
    str
}

#[test]
fn test_server() {
    let server = Server::new().expect("Failed to create server");
    let url = server.url().expect("Failed to get server url");
    assert!(!url.is_empty());
}

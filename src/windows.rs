use crate::{AutoLaunch, Result};
use std::io;
use windows_registry::{Key, CURRENT_USER, LOCAL_MACHINE};
use windows_result::HRESULT;

const AL_REGKEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const TASK_MANAGER_OVERRIDE_REGKEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const TASK_MANAGER_OVERRIDE_ENABLED_VALUE: [u8; 12] = [
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const E_FILENOTFOUND: HRESULT = HRESULT::from_win32(0x80070002_u32);

/// Windows implement
impl AutoLaunch {
    /// Create a new AutoLaunch instance
    /// - `app_name`: application name
    /// - `app_path`: application path
    /// - `args`: startup args passed to the binary
    /// - `args`: startup args passed to the binary
    ///
    /// ## Notes
    ///
    /// The parameters of `AutoLaunch::new` are different on each platform.
    pub fn new(
        app_name: &str,
        app_path: &str,
        args: &[impl AsRef<str>],
        with_admin: bool,
    ) -> AutoLaunch {
        AutoLaunch {
            app_name: app_name.into(),
            app_path: app_path.into(),
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
            with_admin,
        }
    }

    /// Enable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to set value
    pub fn enable(&self) -> Result<()> {
        self.enable_with_root_key(self.root_key())?;
        Ok(())
    }

    fn enable_with_root_key(&self, root_key: &Key) -> io::Result<()> {
        root_key.create(AL_REGKEY)?.set_string(
            &self.app_name,
            format!("{} {}", &self.app_path, &self.args.join(" ")),
        )?;

        match root_key
            .options()
            .write()
            .open(TASK_MANAGER_OVERRIDE_REGKEY)
        {
            Ok(key) => key.set_bytes(
                &self.app_name,
                windows_registry::Type::Bytes,
                &TASK_MANAGER_OVERRIDE_ENABLED_VALUE,
            )?,
            Err(error) if error.code() == E_FILENOTFOUND => {
                return Ok(());
            }
            Err(error) => {
                return Err(error.into());
            }
        }
        Ok(())
    }

    /// Disable the AutoLaunch setting
    ///
    /// ## Errors
    ///
    /// - failed to open the registry key
    /// - failed to delete value
    pub fn disable(&self) -> Result<()> {
        self.disable_with_root_key(self.root_key())?;
        Ok(())
    }

    fn disable_with_root_key(&self, root_key: &Key) -> io::Result<()> {
        match root_key.options().write().open(AL_REGKEY) {
            Ok(key) => Ok(key.remove_value(&self.app_name)?),
            Err(error) if error.code() == E_FILENOTFOUND => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Check whether the AutoLaunch setting is enabled
    pub fn is_enabled(&self) -> Result<bool> {
        let root_key = self.root_key();
        Ok(self.is_registered(root_key)? && self.is_task_manager_enabled(root_key)?)
    }

    fn is_registered(&self, root_key: &Key) -> io::Result<bool> {
        let registered = match root_key
            .open(AL_REGKEY)
            .and_then(|key| key.get_string(&self.app_name))
        {
            Ok(_) => true,
            Err(error) if error.code() == E_FILENOTFOUND => false,
            Err(error) => {
                return Err(error.into());
            }
        };
        Ok(registered)
    }

    fn is_task_manager_enabled(&self, root_key: &Key) -> io::Result<bool> {
        let task_manager_enabled = match root_key
            .open(TASK_MANAGER_OVERRIDE_REGKEY)
            .and_then(|key| key.get_value(&self.app_name))
        {
            Ok(value) => last_eight_bytes_all_zeros(&value).unwrap_or(true),
            Err(error) if error.code() == E_FILENOTFOUND => true,
            Err(error) => {
                return Err(error.into());
            }
        };
        Ok(task_manager_enabled)
    }

    fn root_key(&self) -> &Key {
        if self.with_admin {
            LOCAL_MACHINE
        } else {
            CURRENT_USER
        }
    }
}

fn last_eight_bytes_all_zeros(bytes: &[u8]) -> std::result::Result<bool, &str> {
    if bytes.len() < 8 {
        Err("Bytes too short")
    } else {
        Ok(bytes.iter().rev().take(8).all(|v| *v == 0u8))
    }
}

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{Gid, GroupInfo, OsalEvent, Uid, UserError, UserId, UserInfo, UserManager};
use std::fmt;
use tokio::sync::mpsc::Receiver;
use tokio::task::spawn_blocking;

pub struct LinuxUserManager;

impl LinuxUserManager {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxUserManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxUserManager").finish()
    }
}

fn closed_rx() -> Receiver<OsalEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    drop(tx);
    rx
}

fn get_current_user_sync() -> Result<UserInfo, UserError> {
    let uid = unsafe { libc::getuid() };

    let mut pwd_buf = vec![0u8; 4096];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();

    let ret = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            pwd_buf.as_mut_ptr() as *mut libc::c_char,
            pwd_buf.len(),
            &mut result,
        )
    };
    if ret != 0 || result.is_null() {
        return Err(UserError::NotFound(format!("uid {}", uid)));
    }

    let username = unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }
        .to_string_lossy()
        .into_owned();
    let home_dir = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    let shell = unsafe { std::ffi::CStr::from_ptr(pwd.pw_shell) }
        .to_string_lossy()
        .into_owned();
    let gid = pwd.pw_gid;

    Ok(UserInfo {
        uid: Uid(uid),
        gid: Gid(gid),
        username,
        home_dir,
        shell,
    })
}

fn get_user_by_uid_sync(uid: u32) -> Result<UserInfo, UserError> {
    let mut pwd_buf = vec![0u8; 4096];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();

    let ret = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            pwd_buf.as_mut_ptr() as *mut libc::c_char,
            pwd_buf.len(),
            &mut result,
        )
    };
    if ret != 0 || result.is_null() {
        return Err(UserError::NotFound(format!("uid {}", uid)));
    }

    let username = unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }
        .to_string_lossy()
        .into_owned();
    let home_dir = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    let shell = unsafe { std::ffi::CStr::from_ptr(pwd.pw_shell) }
        .to_string_lossy()
        .into_owned();
    let gid = pwd.pw_gid;

    Ok(UserInfo {
        uid: Uid(uid),
        gid: Gid(gid),
        username,
        home_dir,
        shell,
    })
}

fn enumerate_users_sync() -> Result<Vec<UserInfo>, UserError> {
    let content =
        std::fs::read_to_string("/etc/passwd").map_err(|e| UserError::Io(e.to_string()))?;
    let users: Vec<UserInfo> = content.lines().filter_map(parse_passwd_line).collect();
    Ok(users)
}

fn parse_passwd_line(line: &str) -> Option<UserInfo> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 7 {
        return None;
    }
    let username = parts[0].to_string();
    let uid: u32 = parts[2].parse().ok()?;
    let gid: u32 = parts[3].parse().ok()?;
    let home_dir = parts[5].to_string();
    let shell = parts[6].to_string();
    Some(UserInfo {
        uid: Uid(uid),
        gid: Gid(gid),
        username,
        home_dir,
        shell,
    })
}

fn enumerate_groups_sync() -> Result<Vec<GroupInfo>, UserError> {
    let content =
        std::fs::read_to_string("/etc/group").map_err(|e| UserError::Io(e.to_string()))?;
    let groups: Vec<GroupInfo> = content.lines().filter_map(parse_group_line).collect();
    Ok(groups)
}

fn parse_group_line(line: &str) -> Option<GroupInfo> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 4 {
        return None;
    }
    let name = parts[0].to_string();
    let gid: u32 = parts[2].parse().ok()?;
    let members: Vec<String> = if parts[3].is_empty() {
        Vec::new()
    } else {
        parts[3].split(',').map(|s| s.to_string()).collect()
    };
    Some(GroupInfo {
        gid: Gid(gid),
        name,
        members,
    })
}

#[async_trait]
impl UserManager for LinuxUserManager {
    async fn current_user(&self, _ctx: &CapabilityContext) -> Result<UserInfo, UserError> {
        spawn_blocking(get_current_user_sync)
            .await
            .map_err(|e| UserError::Io(e.to_string()))?
    }

    async fn enumerate_users(&self, _ctx: &CapabilityContext) -> Result<Vec<UserInfo>, UserError> {
        spawn_blocking(enumerate_users_sync)
            .await
            .map_err(|e| UserError::Io(e.to_string()))?
    }

    async fn enumerate_groups(
        &self,
        _ctx: &CapabilityContext,
    ) -> Result<Vec<GroupInfo>, UserError> {
        spawn_blocking(enumerate_groups_sync)
            .await
            .map_err(|e| UserError::Io(e.to_string()))?
    }

    async fn switch_user(
        &self,
        _ctx: &CapabilityContext,
        _user_id: &UserId,
    ) -> Result<(), UserError> {
        Err(UserError::SwitchFailed("not supported".to_string()))
    }

    async fn get_user_by_uid(
        &self,
        _ctx: &CapabilityContext,
        uid: Uid,
    ) -> Result<UserInfo, UserError> {
        spawn_blocking(move || get_user_by_uid_sync(uid.0))
            .await
            .map_err(|e| UserError::Io(e.to_string()))?
    }

    fn events(&self) -> Receiver<OsalEvent> {
        closed_rx()
    }
}

# OSAL — Users Subsystem

User and group identity, authentication, and session management.

---

## Re-exports from `osal_core`

| Item | Description |
|---|---|
| `UserManager` | Async trait for user/group operations |
| `UserError` | User-operation error variants |
| `UserInfo` | User identity information |
| `GroupInfo` | Group identity information |
| `Uid` | Numeric user ID |
| `Gid` | Numeric group ID |

## Unique types

- `Credential` — authentication credential (password, key, token, or none)
- `UserSession` — active user session with access token

## DefaultUserManager

A no-op implementation of `UserManager` that returns `UserError::Other` for
every operation. Useful during early development.

## Events

User login/logout events are part of `OsalEvent` in `osal_core`:
- `OsalEvent::UserLoggedIn { user_id, timestamp }`
- `OsalEvent::UserLoggedOut { user_id, timestamp }`

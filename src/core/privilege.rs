use std::sync::Mutex;
use std::time::{Duration, Instant};

static ELEVATION_STATE: Mutex<Option<ElevationState>> = Mutex::new(None);

const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageMode {
    User,
    System,
}

impl std::fmt::Display for PackageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageMode::User => write!(f, "User"),
            PackageMode::System => write!(f, "System"),
        }
    }
}

impl Default for PackageMode {
    fn default() -> Self {
        Self::User
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorType {
    Sudo,
    Doas,
    Pkexec,
}

impl std::fmt::Display for ElevatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElevatorType::Sudo => write!(f, "sudo"),
            ElevatorType::Doas => write!(f, "doas"),
            ElevatorType::Pkexec => write!(f, "pkexec"),
        }
    }
}

struct ElevationState {
    elevator: ElevatorType,
    elevated_at: Instant,
    cache_duration: Duration,
}

pub struct PrivilegeManager {
    cache_duration: Duration,
}

impl PrivilegeManager {
    pub fn new() -> Self {
        Self {
            cache_duration: DEFAULT_CACHE_DURATION,
        }
    }

    pub fn with_cache_duration(duration: Duration) -> Self {
        Self {
            cache_duration: duration,
        }
    }

    pub fn detect_elevator() -> Option<ElevatorType> {
        if which::which("pkexec").is_ok() {
            Some(ElevatorType::Pkexec)
        } else if which::which("sudo").is_ok() {
            Some(ElevatorType::Sudo)
        } else if which::which("doas").is_ok() {
            Some(ElevatorType::Doas)
        } else {
            None
        }
    }

    pub fn is_elevation_cached(&self) -> bool {
        let state = ELEVATION_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            s.elevated_at.elapsed() < s.cache_duration
        } else {
            false
        }
    }

    pub fn cached_elevator(&self) -> Option<ElevatorType> {
        let state = ELEVATION_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            if s.elevated_at.elapsed() < s.cache_duration {
                return Some(s.elevator);
            }
        }
        None
    }

    pub fn cache_elevation(&self, elevator: ElevatorType) {
        let mut state = ELEVATION_STATE.lock().unwrap();
        *state = Some(ElevationState {
            elevator,
            elevated_at: Instant::now(),
            cache_duration: self.cache_duration,
        });
    }

    pub fn clear_cache() {
        let mut state = ELEVATION_STATE.lock().unwrap();
        *state = None;
    }

    pub fn time_until_expiry(&self) -> Option<Duration> {
        let state = ELEVATION_STATE.lock().unwrap();
        if let Some(ref s) = *state {
            let elapsed = s.elevated_at.elapsed();
            if elapsed < s.cache_duration {
                return Some(s.cache_duration - elapsed);
            }
        }
        None
    }

    pub fn needs_elevation(mode: PackageMode) -> bool {
        matches!(mode, PackageMode::System)
    }

    pub fn prepare_command(
        &self,
        mode: PackageMode,
        mut cmd: std::process::Command,
    ) -> Result<std::process::Command, PrivilegeError> {
        if mode == PackageMode::User {
            return Ok(cmd);
        }

        let elevator = self
            .cached_elevator()
            .or_else(Self::detect_elevator)
            .ok_or(PrivilegeError::NoElevatorFound)?;

        let program = cmd.get_program().to_string_lossy().to_string();
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        let mut elevated_cmd = match elevator {
            ElevatorType::Sudo => {
                let mut c = std::process::Command::new("sudo");
                if self.is_elevation_cached() {
                    c.arg("-n");
                }
                c.arg(&program);
                for arg in &args {
                    c.arg(arg);
                }
                c
            }
            ElevatorType::Doas => {
                let mut c = std::process::Command::new("doas");
                c.arg(&program);
                for arg in &args {
                    c.arg(arg);
                }
                c
            }
            ElevatorType::Pkexec => {
                let mut c = std::process::Command::new("pkexec");
                c.arg(&program);
                for arg in &args {
                    c.arg(arg);
                }
                c
            }
        };

        if let Some(dir) = cmd.get_current_dir() {
            elevated_cmd.current_dir(dir);
        }

        for (key, val) in cmd.get_envs() {
            if let Some(v) = val {
                elevated_cmd.env(key, v);
            } else {
                elevated_cmd.env_remove(key);
            }
        }

        Ok(elevated_cmd)
    }

    pub fn mark_elevated(&self) {
        if let Some(elevator) = Self::detect_elevator() {
            self.cache_elevation(elevator);
        }
    }
}

impl Default for PrivilegeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrivilegeError {
    #[error("No privilege elevation tool found (sudo, doas, or pkexec required)")]
    NoElevatorFound,
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Operation cancelled by user")]
    Cancelled,
    #[error("Permission denied")]
    PermissionDenied,
}

pub fn run_elevated(
    mode: PackageMode,
    program: &str,
    args: &[&str],
) -> Result<std::process::Output, PrivilegeError> {
    if mode == PackageMode::User {
        return std::process::Command::new(program)
            .args(args)
            .output()
            .map_err(|e| PrivilegeError::AuthenticationFailed(e.to_string()));
    }

    let manager = PrivilegeManager::new();
    let elevator = manager
        .cached_elevator()
        .or_else(PrivilegeManager::detect_elevator)
        .ok_or(PrivilegeError::NoElevatorFound)?;

    let output = match elevator {
        ElevatorType::Sudo => {
            let mut cmd = std::process::Command::new("sudo");
            if manager.is_elevation_cached() {
                cmd.arg("-n");
            }
            cmd.arg(program).args(args).output()
        }
        ElevatorType::Doas => std::process::Command::new("doas")
            .arg(program)
            .args(args)
            .output(),
        ElevatorType::Pkexec => {
            let full_cmd = format!("{} {}", program, args.join(" "));
            let mut cmd = std::process::Command::new("pkexec");
            cmd.args(["/bin/sh", "-c", &full_cmd]);
            cmd.env_remove("SHELL");
            if let Ok(display) = std::env::var("DISPLAY") {
                cmd.env("DISPLAY", &display);
            }
            if let Ok(xauth) = std::env::var("XAUTHORITY") {
                cmd.env("XAUTHORITY", &xauth);
            }
            cmd.output()
        }
    };

    let output = output.map_err(|e| PrivilegeError::AuthenticationFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("incorrect password") || stderr.contains("Sorry") {
            return Err(PrivilegeError::AuthenticationFailed(
                "Incorrect password".into(),
            ));
        }
        if stderr.contains("cancelled") {
            return Err(PrivilegeError::Cancelled);
        }
    }

    manager.mark_elevated();
    Ok(output)
}

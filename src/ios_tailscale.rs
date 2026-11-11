//! Tailscale zabudovany primo v aplikaci (tsnet pres libtailscale) - jen iOS.
//!
//! iPad s iOS 12 neumi systemovou VPN Tailscale. Tato cast spusti Tailscale
//! uvnitr aplikace jako samostatne zarizeni v tailnetu a zapne mistni SOCKS5
//! proxy (jen na 127.0.0.1). RustDesk pak vsechna spojeni vede pres ni, takze
//! se da pripojit primo na Tailscale adresu Macu a provoz je sifrovany
//! WireGuardem od iPadu az po Mac.
//!
//! Stav se uklada do mistni volby `ts-status`, kterou zobrazuje uvodni obrazovka.
//! Vse, co aplikace vypisuje na stderr (vcetne prihlasovaciho odkazu Tailscale),
//! jde do souboru `<Documents>/tailscale/app.log`.

use hbb_common::{
    config::{Config, LocalConfig, Socks5Server},
    libc, log,
};
use std::{
    ffi::{CStr, CString},
    os::raw::{c_char, c_int},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

extern "C" {
    fn tailscale_new() -> c_int;
    fn tailscale_set_dir(sd: c_int, dir: *const c_char) -> c_int;
    fn tailscale_set_hostname(sd: c_int, hostname: *const c_char) -> c_int;
    fn tailscale_up(sd: c_int) -> c_int;
    fn tailscale_loopback(
        sd: c_int,
        addr_out: *mut c_char,
        addrlen: usize,
        proxy_cred_out: *mut c_char,
        local_api_cred_out: *mut c_char,
    ) -> c_int;
    fn tailscale_getips(sd: c_int, buf: *mut c_char, buflen: usize) -> c_int;
    fn tailscale_errmsg(sd: c_int, buf: *mut c_char, buflen: usize) -> c_int;
}

const STATUS_KEY: &str = "ts-status";
const LOGIN_URL_PREFIX: &str = "https://login.tailscale.com/a/";
static STARTED: AtomicBool = AtomicBool::new(false);
static UP: AtomicBool = AtomicBool::new(false);

fn set_status(s: &str) {
    log::info!("tailscale: {}", s);
    LocalConfig::set_option(STATUS_KEY.to_owned(), s.to_owned());
}

fn buf_to_string(buf: &[u8]) -> String {
    CStr::from_bytes_until_nul(buf)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn errmsg(sd: c_int) -> String {
    let mut buf = [0u8; 512];
    unsafe { tailscale_errmsg(sd, buf.as_mut_ptr() as *mut c_char, buf.len()) };
    buf_to_string(&buf)
}

/// Posledni prihlasovaci odkaz, ktery Tailscale vypsal do logu.
fn find_login_url(log_path: &str) -> Option<String> {
    let text = std::fs::read_to_string(log_path).ok()?;
    let pos = text.rfind(LOGIN_URL_PREFIX)?;
    let url: String = text[pos..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    Some(url)
}

pub fn start(app_dir: &str) {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = format!("{}/tailscale", app_dir);
    std::fs::create_dir_all(&dir).ok();
    let log_path = format!("{}/app.log", dir);

    // stderr (Rust log i Go log z Tailscale) -> soubor, aby sel precist pres kabel
    if let Ok(c_log) = CString::new(log_path.clone()) {
        unsafe {
            let fd = libc::open(
                c_log.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o644 as libc::c_uint,
            );
            if fd >= 0 {
                libc::dup2(fd, 2);
                libc::close(fd);
            }
        }
    }

    // dokud neni Tailscale nahore, nesmi RustDesk pouzit starou proxy z minula
    Config::set_socks(None);
    set_status("Tailscale: startuji…");

    let watcher_log = log_path.clone();
    std::thread::spawn(move || {
        // dokud bezi tailscale_up, hlidame log kvuli prihlasovacimu odkazu
        while !UP.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(2));
            if UP.load(Ordering::SeqCst) {
                break;
            }
            if let Some(url) = find_login_url(&watcher_log) {
                let s = format!("Tailscale: prihlaste iPad na adrese {}", url);
                if LocalConfig::get_option(STATUS_KEY) != s {
                    set_status(&s);
                }
            }
        }
    });

    std::thread::spawn(move || unsafe {
        let sd = tailscale_new();
        let (Ok(c_dir), Ok(c_host)) = (CString::new(dir), CString::new("ipad-mini")) else {
            set_status("Tailscale: chyba (cesta)");
            return;
        };
        tailscale_set_dir(sd, c_dir.as_ptr());
        tailscale_set_hostname(sd, c_host.as_ptr());
        if tailscale_up(sd) != 0 {
            UP.store(true, Ordering::SeqCst);
            set_status(&format!("Tailscale: chyba – {}", errmsg(sd)));
            return;
        }
        let mut addr = [0u8; 128];
        let mut proxy_cred = [0u8; 33];
        let mut local_cred = [0u8; 33];
        if tailscale_loopback(
            sd,
            addr.as_mut_ptr() as *mut c_char,
            addr.len(),
            proxy_cred.as_mut_ptr() as *mut c_char,
            local_cred.as_mut_ptr() as *mut c_char,
        ) != 0
        {
            UP.store(true, Ordering::SeqCst);
            set_status(&format!("Tailscale: chyba proxy – {}", errmsg(sd)));
            return;
        }
        UP.store(true, Ordering::SeqCst);
        Config::set_socks(Some(Socks5Server {
            proxy: format!("socks5://{}", buf_to_string(&addr)),
            username: "tsnet".to_owned(),
            password: buf_to_string(&proxy_cred),
        }));
        let mut ips = [0u8; 256];
        tailscale_getips(sd, ips.as_mut_ptr() as *mut c_char, ips.len());
        let ip = buf_to_string(&ips);
        let ip = ip.split(',').next().unwrap_or_default().to_owned();
        set_status(&format!("Tailscale: pripojeno ({})", ip));
    });
}

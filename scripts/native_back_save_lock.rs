// Test fixture only. Hold a real SQLite writer in the dedicated Android package.
// No writes are committed; EOF, a release line, or 12 seconds rolls it back.
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::{self, BufRead, Write};
use std::ptr;
use std::sync::mpsc;
use std::time::Duration;

#[link(name = "sqlite3", kind = "static")]
unsafe extern "C" {
    fn sqlite3_open_v2(path: *const c_char, db: *mut *mut c_void, flags: c_int, vfs: *const c_char) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_exec(db: *mut c_void, sql: *const c_char, callback: *const c_void, arg: *mut c_void, error: *mut *mut c_char) -> c_int;
    fn sqlite3_prepare_v2(db: *mut c_void, sql: *const c_char, size: c_int, stmt: *mut *mut c_void, tail: *mut *const c_char) -> c_int;
    fn sqlite3_step(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_int(stmt: *mut c_void, index: c_int) -> c_int;
    fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
}

struct Database(*mut c_void);
impl Drop for Database {
    fn drop(&mut self) {
        unsafe {
            sqlite3_exec(self.0, c"ROLLBACK".as_ptr(), ptr::null(), ptr::null_mut(), ptr::null_mut());
            sqlite3_close(self.0);
        }
    }
}

fn main() {
    assert_eq!(std::env::current_dir().unwrap().to_str().unwrap(),
        "/data/user/0/io.github.develata.rssreader.backpolicytest");
    let mut db = ptr::null_mut();
    // READWRITE only: never create a missing library.
    assert_eq!(unsafe { sqlite3_open_v2(c"files/RSS-Reader/library.sqlite3".as_ptr(), &mut db, 2, ptr::null()) }, 0);
    let db = Database(db);
    let sql: &CStr = c"SELECT (SELECT count(*) FROM entries)=100 AND (SELECT count(*) FROM entries WHERE title=printf('阅读基线文章 %05d — Reading baseline',id))=100 AND (SELECT count(*) FROM feeds)=6 AND (SELECT count(*) FROM feeds WHERE url GLOB 'http://127.0.0.1:9299/reading-baseline/[1-6].xml')=6";
    let mut stmt = ptr::null_mut();
    assert_eq!(unsafe { sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) }, 0);
    let verified = unsafe { sqlite3_step(stmt) == 100 && sqlite3_column_int(stmt, 0) == 1 };
    unsafe { sqlite3_finalize(stmt); }
    assert!(verified, "refuse a non-fixture library");
    assert_eq!(unsafe { sqlite3_exec(db.0, c"BEGIN IMMEDIATE".as_ptr(), ptr::null(), ptr::null_mut(), ptr::null_mut()) }, 0);
    println!("LOCKED");
    io::stdout().flush().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().lock().read_line(&mut line);
        let _ = send.send(());
    });
    let released = receive.recv_timeout(Duration::from_secs(12)).is_ok();
    drop(db);
    println!("{}", if released { "RELEASED" } else { "TIMED_OUT" });
    if !released { std::process::exit(2); }
}

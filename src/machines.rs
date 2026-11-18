//! Machines by what they are, not by the client IDs their setups made. One machine can hold
//! several client IDs (a setup per OS account, or one made again under another team), so the
//! engine's inventory is read into a table of each client ID's machine: the same hardware serial
//! is one machine, else the same host name; it is named by its host name, and two machines that
//! share a name are told apart by the end of their serials. Pages ask [`name`] for a client ID's
//! machine; one the inventory does not know is its client ID cut short.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use crate::model::Device;

static NAMES: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// A host name without its domain and in one case, as two setups of one machine may report it
/// (`Mac-1.local`, `mac-1`).
fn host_key(host: &str) -> String {
    host.trim().split('.').next().unwrap_or(host).to_lowercase()
}

/// A host name as shown: its domain left off.
fn host_name(host: &str) -> String {
    host.trim()
        .strip_suffix(".local")
        .unwrap_or(host.trim())
        .to_owned()
}

/// What firmware reports when no serial was set: not a serial.
fn real_serial(serial: Option<&str>) -> Option<String> {
    let s = serial?.trim();
    let placeholder = [
        "",
        "0",
        "none",
        "default string",
        "not specified",
        "not applicable",
        "system serial number",
        "to be filled by o.e.m.",
    ]
    .contains(&s.to_lowercase().as_str());
    (!placeholder).then(|| s.to_uppercase())
}

/// A client ID (`<team>/<uuid>`) with its UUID cut to the first part; any other ID as it is.
pub fn short(client: &str) -> String {
    let (team, id) = client.split_once('/').unwrap_or(("", client));
    let uuid = id.len() == 36 && id.matches('-').count() == 4;
    match (uuid, team) {
        (false, _) => client.to_owned(),
        (true, "") => id[..8].to_owned(),
        (true, team) => format!("{team}/{}", &id[..8]),
    }
}

/// Each client ID's machine name, from the inventory's devices.
pub fn names(devices: &[Device]) -> HashMap<String, String> {
    // A host name that a device with a serial reported stands for that serial's machine, so a
    // setup that could not read the serial still joins it.
    let mut serial_of_host: HashMap<String, String> = HashMap::new();
    for d in devices {
        if let (Some(serial), Some(host)) = (real_serial(d.serial.as_deref()), &d.hostname) {
            serial_of_host.entry(host_key(host)).or_insert(serial);
        }
    }
    let key_of = |d: &Device| -> String {
        if let Some(serial) = real_serial(d.serial.as_deref()) {
            return format!("serial:{serial}");
        }
        match &d.hostname {
            Some(host) => match serial_of_host.get(&host_key(host)) {
                Some(serial) => format!("serial:{serial}"),
                None => format!("host:{}", host_key(host)),
            },
            None => format!("id:{}", d.client_id),
        }
    };
    // Each machine's name: the host name its latest-heard client reported.
    let mut machines: HashMap<String, (Option<i64>, String, Option<String>)> = HashMap::new();
    for d in devices {
        let key = key_of(d);
        let name = d
            .hostname
            .as_deref()
            .map(host_name)
            .unwrap_or_else(|| short(&d.client_id));
        let heard = d.last_seen.or(d.updated_at);
        let serial = real_serial(d.serial.as_deref());
        let entry = machines
            .entry(key)
            .or_insert((heard, name.clone(), serial.clone()));
        if heard > entry.0 {
            *entry = (heard, name, serial.or(entry.2.clone()));
        }
    }
    // Two machines with one name: each told apart by its serial's end.
    let mut count: HashMap<String, usize> = HashMap::new();
    for (_, name, _) in machines.values() {
        *count.entry(name.to_lowercase()).or_default() += 1;
    }
    let shown = |key: &str| -> String {
        let (_, name, serial) = &machines[key];
        match (count[&name.to_lowercase()] > 1, serial) {
            (true, Some(serial)) => {
                format!("{name} · {}", &serial[serial.len().saturating_sub(4)..])
            }
            _ => name.clone(),
        }
    };
    devices
        .iter()
        .map(|d| (d.client_id.clone(), shown(&key_of(d))))
        .collect()
}

/// Takes the inventory's devices in, for [`name`].
pub fn learn(devices: &[Device]) {
    let table = names(devices);
    if let Ok(mut names) = NAMES.write() {
        *names = table;
    }
}

/// The machine of a client ID: its name, or the ID cut short while the inventory does not know it.
pub fn name(client: &str) -> String {
    NAMES
        .read()
        .ok()
        .and_then(|n| n.get(client).cloned())
        .unwrap_or_else(|| short(client))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, host: Option<&str>, serial: Option<&str>, seen: i64) -> Device {
        Device {
            client_id: id.into(),
            hostname: host.map(str::to_owned),
            serial: serial.map(str::to_owned),
            last_seen: Some(seen),
            ..Device::default()
        }
    }

    #[test]
    fn one_machine_by_its_serial_else_its_host_name() {
        let names = names(&[
            // One Mac: set up as personal, again as default, and once where the serial was not read.
            device(
                "personal/555cfd71-7612-47a9-8d99-4070aaaaaaaa",
                Some("Bob-Book.local"),
                Some("C02XK1"),
                1,
            ),
            device(
                "default/7a345d01-5f75-4ebd-a82f-b8b256c7cccc",
                Some("bobbook"),
                Some("c02xk1"),
                2,
            ),
            device("lab/1", Some("BOB-BOOK"), None, 3),
            // Another machine, named alike, its own serial.
            device("default/9", Some("BOB-BOOK.local"), Some("FVFZZ9QW"), 4),
            // A machine the inventory knows only by its ID.
            device("team/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", None, None, 5),
        ]);
        let mac = &names["personal/555cfd71-7612-47a9-8d99-4070aaaaaaaa"];
        assert_eq!(&names["default/7a345d01-5f75-4ebd-a82f-b8b256c7cccc"], mac);
        assert_eq!(&names["lab/1"], mac);
        assert_ne!(&names["default/9"], mac);
        assert_eq!(names["default/9"], "BOB-BOOK · Z9QW");
        assert_eq!(mac, "BOB-BOOK · 2XK1");
        assert_eq!(
            names["team/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"],
            "team/aaaaaaaa"
        );
    }

    #[test]
    fn a_client_id_unknown_is_cut_short() {
        assert_eq!(
            short("default/7a345d01-5f75-4ebd-a82f-b8b256aa0000"),
            "default/7a345d01"
        );
        assert_eq!(short("lab/device-1"), "lab/device-1");
    }
}

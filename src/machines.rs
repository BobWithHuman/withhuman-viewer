//! Machines by what they are, not by the client IDs their setups made. One machine can hold
//! several client IDs (a setup per OS account, or one made again under another team), so the
//! engine's inventory is read into a table of each client ID's machine: the same hardware serial
//! is one machine, else the same host name; it is named by its host name, and two machines that
//! share a name are told apart by the end of their serials. A client ID no setup registered (one
//! a script made up for its hooks) is the machine whose sessions ran in the same folders, when one
//! machine alone did. Pages ask [`name`] for a client ID's machine; one nothing tells of is its
//! client ID cut short.

use std::collections::{BTreeSet, HashMap};
use std::sync::{LazyLock, RwLock};

use crate::model::{Device, Session};

/// What the names are made from: the inventory's devices, and the folders each client ID's
/// sessions ran in; and the names made.
#[derive(Default)]
struct Known {
    devices: Vec<Device>,
    folders: HashMap<String, BTreeSet<String>>,
    names: HashMap<String, String>,
}

static KNOWN: LazyLock<RwLock<Known>> = LazyLock::new(|| RwLock::new(Known::default()));

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

/// Folders every machine has, which tell nothing of which machine it is.
const SHARED_FOLDERS: [&str; 7] = [
    "tmp",
    "private",
    "var",
    "windows",
    "temp",
    "program files",
    "programdata",
];

/// Where a session's working folder belongs, in one case and with `/`: the user's home
/// (`users/ana`, `home/ana`, `c:/users/ana`), else a Windows drive's first folder
/// (`c:/withhuman`), else the first two folders. None for a folder too short, or one every
/// machine has (a temporary or system folder).
fn folder_root(cwd: &str) -> Option<String> {
    let path = cwd.trim().replace('\\', "/").to_lowercase();
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let drive = parts
        .first()
        .is_some_and(|p| p.len() == 2 && p.ends_with(':'));
    let at = usize::from(drive);
    let first = *parts.get(at)?;
    if SHARED_FOLDERS.contains(&first) {
        return None;
    }
    let take = if first == "users" || first == "home" {
        at + 2
    } else if drive {
        at + 1
    } else {
        2
    };
    (parts.len() >= take).then(|| parts[..take].join("/"))
}

/// The machine of each client ID no setup registered (no host name, no serial), from the folders
/// its sessions ran in: the one machine whose registered client IDs' sessions ran under the same
/// roots. None when no machine did, or more than one.
fn by_folder(
    devices: &[Device],
    folders: &HashMap<String, BTreeSet<String>>,
    names: &HashMap<String, String>,
) -> HashMap<String, String> {
    let registered = |client: &str| {
        devices
            .iter()
            .any(|d| d.client_id == client && registered(d))
    };
    let mut machines_of: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for (client, roots) in folders {
        let Some(machine) = names.get(client).filter(|_| registered(client)) else {
            continue;
        };
        for root in roots {
            machines_of.entry(root).or_default().insert(machine);
        }
    }
    folders
        .iter()
        .filter(|(client, _)| !registered(client))
        .filter_map(|(client, roots)| {
            let found: BTreeSet<&str> = roots
                .iter()
                .filter_map(|root| machines_of.get(root.as_str()))
                .flatten()
                .copied()
                .collect();
            let machine = found.first().filter(|_| found.len() == 1)?;
            Some((client.clone(), (*machine).to_owned()))
        })
        .collect()
}

/// Makes the names again from what is known.
fn remake(known: &mut Known) {
    let mut table = names(&known.devices);
    let found = by_folder(&known.devices, &known.folders, &table);
    table.extend(found);
    known.names = table;
}

/// Takes the inventory's devices in, for [`name`].
pub fn learn(devices: &[Device]) {
    if let Ok(mut known) = KNOWN.write() {
        known.devices = devices.to_vec();
        remake(&mut known);
    }
}

/// Takes in the folders the sessions ran in, for [`name`].
pub fn learn_folders(sessions: &[Session]) {
    let mut folders: HashMap<String, BTreeSet<String>> = HashMap::new();
    for s in sessions {
        let root = s.cwd.as_deref().and_then(folder_root);
        if let (Some(client), Some(root)) = (s.client.as_ref(), root) {
            folders.entry(client.clone()).or_default().insert(root);
        }
    }
    if let Ok(mut known) = KNOWN.write() {
        known.folders = folders;
        remake(&mut known);
    }
}

/// The machine a session with no client ID ran on, from its working folder: the one machine
/// whose registered client IDs' sessions ran under the same root; none when no machine did, or
/// more than one.
pub fn of_folder(cwd: &str) -> Option<String> {
    let root = folder_root(cwd)?;
    let known = KNOWN.read().ok()?;
    let machines: BTreeSet<&String> = known
        .folders
        .iter()
        .filter(|(client, roots)| {
            roots.contains(&root)
                && known
                    .devices
                    .iter()
                    .any(|d| &d.client_id == *client && registered(d))
        })
        .filter_map(|(client, _)| known.names.get(client))
        .collect();
    let machine = machines.first().filter(|_| machines.len() == 1)?;
    Some((*machine).clone())
}

/// Whether a setup registered the device: it reported a host name or a serial.
fn registered(d: &Device) -> bool {
    d.hostname.is_some() || real_serial(d.serial.as_deref()).is_some()
}

/// The registered client ID that stands for a client ID's machine in the inventory: itself when
/// registered, else the machine's latest-heard registered client ID; none when the machine is
/// unknown or has none.
pub fn device(client: &str) -> Option<String> {
    let known = KNOWN.read().ok()?;
    let name = known.names.get(client)?;
    known
        .devices
        .iter()
        .filter(|d| registered(d) && known.names.get(&d.client_id) == Some(name))
        .max_by_key(|d| (d.client_id == client, d.last_seen.or(d.updated_at)))
        .map(|d| d.client_id.clone())
}

/// The machine of a client ID: its name, or the ID cut short while nothing tells of it.
pub fn name(client: &str) -> String {
    KNOWN
        .read()
        .ok()
        .and_then(|k| k.names.get(client).cloned())
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
    fn a_made_up_client_id_is_the_machine_that_ran_in_its_folders() {
        let bob = "personal/555cfd71-7612-47a9-8d99-407d959e3405";
        let mac = "default/6427f4b3-f93e-40df-b8a1-85b3a48477b0";
        let devices = [
            device(bob, Some("BoBbook"), Some("5CD5479KD6"), 1),
            device(mac, Some("seonung-mac-2.local"), Some("FV39N23V5H"), 2),
            // A script's client ID: seen by the hooks, never registered.
            device("intent-drift-lab/s3", None, None, 3),
        ];
        let table = names(&devices);
        let root = |s: &str| folder_root(s).unwrap();
        let folders: HashMap<String, BTreeSet<String>> = [
            (bob, root("C:\\WithHuman\\demo-ec2")),
            (mac, root("/Users/bagseon-ung/withhuman-viewer")),
            (
                "intent-drift-lab/s3",
                root("C:\\WithHuman\\intent-drift-lab\\runs\\s3\\sandbox"),
            ),
            // Its folders are no machine's: unknown still.
            ("intent-drift-lab/x", root("D:\\elsewhere\\x")),
        ]
        .into_iter()
        .map(|(c, r)| (c.to_owned(), BTreeSet::from([r])))
        .collect();
        let found = by_folder(&devices, &folders, &table);
        assert_eq!(found["intent-drift-lab/s3"], "BoBbook");
        assert!(!found.contains_key("intent-drift-lab/x"));
        // A registered machine is never moved, whatever folders it shares.
        assert!(!found.contains_key(mac) && !found.contains_key(bob));
    }

    #[test]
    fn a_folder_belongs_to_its_home_or_its_first_folders() {
        let root = |s| folder_root(s);
        assert_eq!(root("C:\\WithHuman\\a\\b").as_deref(), Some("c:/withhuman"));
        assert_eq!(
            root("C:\\Users\\qkaeh\\src\\x").as_deref(),
            Some("c:/users/qkaeh")
        );
        assert_eq!(root("/Users/bae/work/x").as_deref(), Some("users/bae"));
        assert_eq!(root("/home/ubuntu/x").as_deref(), Some("home/ubuntu"));
        assert_eq!(root("/opt/lab/x").as_deref(), Some("opt/lab"));
        // Every machine has these, or they are too short to tell.
        for none in [
            "/tmp/x/y",
            "/private/var/folders/x",
            "C:\\Windows\\System32",
            "C:\\",
            "/Users",
        ] {
            assert_eq!(root(none), None, "{none}");
        }
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

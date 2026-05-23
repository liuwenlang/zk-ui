use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStat {
    pub czxid: i64,
    pub mzxid: i64,
    pub ctime: i64,
    pub mtime: i64,
    pub version: i32,
    pub cversion: i32,
    pub aversion: i32,
    pub ephemeral_owner: i64,
    pub data_length: i32,
    pub num_children: i32,
    pub pzxid: i64,
}

impl From<zookeeper::Stat> for NodeStat {
    fn from(s: zookeeper::Stat) -> Self {
        Self {
            czxid: s.czxid,
            mzxid: s.mzxid,
            ctime: s.ctime,
            mtime: s.mtime,
            version: s.version,
            cversion: s.cversion,
            aversion: s.aversion,
            ephemeral_owner: s.ephemeral_owner,
            data_length: s.data_length,
            num_children: s.num_children,
            pzxid: s.pzxid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    pub scheme: String,
    pub id: String,
    pub perms: u32,
}

impl From<&zookeeper::Acl> for AclEntry {
    fn from(a: &zookeeper::Acl) -> Self {
        Self {
            scheme: a.scheme.clone(),
            id: a.id.clone(),
            perms: perm_to_bits(a.perms),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CreateMode {
    Persistent,
    Ephemeral,
    PersistentSequential,
    EphemeralSequential,
}

impl CreateMode {
    pub fn label(self) -> &'static str {
        match self {
            CreateMode::Persistent => "Persistent",
            CreateMode::Ephemeral => "Ephemeral",
            CreateMode::PersistentSequential => "Persistent Sequential",
            CreateMode::EphemeralSequential => "Ephemeral Sequential",
        }
    }
}

pub fn perm_to_bits(p: zookeeper::Permission) -> u32 {
    let mut bits = 0u32;
    if p.can(zookeeper::Permission::READ) { bits |= 1; }
    if p.can(zookeeper::Permission::WRITE) { bits |= 2; }
    if p.can(zookeeper::Permission::CREATE) { bits |= 4; }
    if p.can(zookeeper::Permission::DELETE) { bits |= 8; }
    if p.can(zookeeper::Permission::ADMIN) { bits |= 16; }
    bits
}

pub fn bits_to_perm(bits: u32) -> zookeeper::Permission {
    let mut p = zookeeper::Permission::NONE;
    if bits & 1 != 0 { p = p | zookeeper::Permission::READ; }
    if bits & 2 != 0 { p = p | zookeeper::Permission::WRITE; }
    if bits & 4 != 0 { p = p | zookeeper::Permission::CREATE; }
    if bits & 8 != 0 { p = p | zookeeper::Permission::DELETE; }
    if bits & 16 != 0 { p = p | zookeeper::Permission::ADMIN; }
    p
}

pub fn perm_string(perms: u32) -> String {
    let mut s = String::new();
    if perms & 1 != 0 { s.push('r'); }
    if perms & 2 != 0 { s.push('w'); }
    if perms & 4 != 0 { s.push('c'); }
    if perms & 8 != 0 { s.push('d'); }
    if perms & 16 != 0 { s.push('a'); }
    if s.is_empty() { s.push('-'); }
    s
}

// Multi-line source documents for line-by-line Scanner workloads.
//
// vscode-textmate tokenizes a document one line at a time, handing each line
// (with its trailing "\n") to the scanner once. The RegSet fallback memo only
// remembers the most recent string, so walking distinct lines keeps every
// line cold -- unlike benches that re-tokenize one line, where the memo from
// the previous iteration is reused.

#[allow(dead_code)]
pub const TYPESCRIPT_DOCUMENT: &str = r#"import { readFile } from "node:fs/promises";
import type { User, Session } from "./types";

export interface FetchOptions {
  limit?: number;
  offset?: number;
  signal?: AbortSignal;
}

export async function fetchUsers(opts: FetchOptions = {}): Promise<User[]> {
  const { limit = 100, offset = 0 } = opts;
  const url = `/api/users?limit=${limit}&offset=${offset}`;
  const res = await fetch(url, { signal: opts.signal });
  if (!res.ok) {
    throw new Error(`Request failed: ${res.status} ${res.statusText}`);
  }
  const data = (await res.json()) as { users: User[] };
  return data.users.filter((u) => u.active && !u.deleted);
}

class SessionStore<T extends Session = Session> {
  private readonly sessions = new Map<string, T>();
  constructor(private ttlMs: number = 60_000) {}
  get(id: string): T | undefined {
    const s = this.sessions.get(id);
    return s && Date.now() - s.createdAt < this.ttlMs ? s : undefined;
  }
}
"#;

#[allow(dead_code)]
pub const RUST_DOCUMENT: &str = r#"use std::collections::HashMap;
use std::fmt::{self, Display};

/// A simple key-value cache with a capacity limit.
#[derive(Debug, Clone, Default)]
pub struct Cache<K, V> {
    map: HashMap<K, V>,
    capacity: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V> Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self { map: HashMap::with_capacity(capacity), capacity }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.map.len() >= self.capacity && !self.map.contains_key(&key) {
            return None;
        }
        self.map.insert(key, value)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut c: Cache<&str, u32> = Cache::new(16);
    for (i, w) in ["a", "b", "c"].iter().enumerate() {
        c.insert(w, i as u32 * 2 + 1);
    }
    println!("{:?} {}", c, 0x1F_u8 as char);
    Ok(())
}
"#;

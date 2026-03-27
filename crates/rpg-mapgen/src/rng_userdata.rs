//! Lua userdata wrapper for [`SeededRng`].
//!
//! Exposes the deterministic PRNG to Lua scripts so that generation logic
//! can call it without leaving the scripting layer.

use std::cell::RefCell;

use mlua::{UserData, UserDataMethods};
use rpg_engine::rng::SeededRng;

// ─── LuaRng ───────────────────────────────────────────────────────────────────

/// A [`SeededRng`] exposed to Lua as a userdata object.
///
/// Methods are available on the Lua side as `rng:method(args)`.
/// Interior mutability via [`RefCell`] is required because mlua passes
/// the userdata as a shared reference.
pub struct LuaRng(pub RefCell<SeededRng>);

impl LuaRng {
    /// Creates a new [`LuaRng`] from a raw 32-byte seed.
    pub fn new(seed: [u8; 32]) -> Self {
        Self(RefCell::new(SeededRng::from_bytes(seed)))
    }
}

impl UserData for LuaRng {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // rng:next_f64() → number in [0.0, 1.0)
        methods.add_method("next_f64", |_, this, ()| {
            Ok(this.0.borrow_mut().next_f64())
        });

        // rng:random_range_u32(lo, hi) → integer in [lo, hi)
        methods.add_method("random_range_u32", |_, this, (lo, hi): (u32, u32)| {
            Ok(this.0.borrow_mut().random_range_u32(lo..hi))
        });

        // rng:random_range_i32(lo, hi) → integer in [lo, hi)
        methods.add_method("random_range_i32", |_, this, (lo, hi): (i32, i32)| {
            Ok(this.0.borrow_mut().random_range_i32(lo..hi))
        });

        // rng:random_range_f64(lo, hi) → float in [lo, hi)
        methods.add_method("random_range_f64", |_, this, (lo, hi): (f64, f64)| {
            Ok(this.0.borrow_mut().random_range_f64(lo..hi))
        });

        // rng:random_bool(probability) → boolean
        methods.add_method("random_bool", |_, this, prob: f64| {
            Ok(this.0.borrow_mut().random_bool(prob))
        });
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;
    use rpg_engine::rng::keccak256;
    use crate::test_utils::init_tracing;

    #[test]
    fn lua_rng_next_f64_in_range() {
        init_tracing();
        let lua = Lua::new();
        let seed = keccak256("test");
        let rng = lua.create_userdata(LuaRng::new(seed)).unwrap();
        lua.globals().set("rng", rng).unwrap();

        let result: f64 = lua
            .load("return rng:next_f64()")
            .eval()
            .unwrap();
        assert!((0.0..1.0).contains(&result), "next_f64 {result} out of [0,1)");
    }

    #[test]
    fn lua_rng_random_range_u32_in_range() {
        init_tracing();
        let lua = Lua::new();
        let seed = keccak256("range");
        let rng = lua.create_userdata(LuaRng::new(seed)).unwrap();
        lua.globals().set("rng", rng).unwrap();

        for _ in 0..100 {
            let v: u32 = lua
                .load("return rng:random_range_u32(5, 10)")
                .eval()
                .unwrap();
            assert!((5..10).contains(&v), "random_range_u32 {v} out of 5..10");
        }
    }

    #[test]
    fn lua_rng_deterministic_with_same_seed() {
        init_tracing();
        let seed = keccak256("determinism");

        let make_values = || {
            let lua = Lua::new();
            let rng = lua.create_userdata(LuaRng::new(seed)).unwrap();
            lua.globals().set("rng", rng).unwrap();
            let table: mlua::Table = lua
                .load(
                    r#"
                    local t = {}
                    for i = 1, 10 do t[i] = rng:next_f64() end
                    return t
                "#,
                )
                .eval()
                .unwrap();
            let values: Vec<f64> = (1..=10)
                .map(|i| table.get::<f64>(i).unwrap())
                .collect();
            values
        };

        assert_eq!(make_values(), make_values());
    }
}

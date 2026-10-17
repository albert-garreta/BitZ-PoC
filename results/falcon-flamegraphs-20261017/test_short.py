"""Spot-check flame.py's frame-name shortener on names from both profilers."""

import sys
from pathlib import Path

source = (Path(__file__).parent / "flame.py").read_text()
helpers = source[: source.index("summary = {}")].replace("OUT.mkdir(parents=True, exist_ok=True)", "")
sys.argv = ["flame.py", "/tmp", "/tmp"]
namespace = {"__name__": "flame_helpers"}
exec(compile(helpers, "flame.py", "exec"), namespace)
short = namespace["short"]

CASES = {
    "akita_algebra::ring::crt_ntt_repr::convert::<impl akita_algebra::ring::crt_ntt_repr::CyclotomicCrtNtt<W,_,_>>::assign_i8_with_lut":
        "CyclotomicCrtNtt::assign_i8_with_lut",
    "akita_prover::protocol::prove::execution::_<impl akita_prover::protocol::prove::registry::Executor<Cfg> for akita_prover::protocol::prove::registry::TypedExecutor<Cfg,B>>::recursive":
        "<TypedExecutor as Executor>::recursive",
    "<akita_cpu_backend::opaque::recursive::witness::stage_sessions::CpuExtensionOpeningSession<E> as akita_cpu_backend::opaque::eor::ExtensionOpeningSession<E>>::bind_challenge":
        "<CpuExtensionOpeningSession as ExtensionOpeningSession>::bind_challenge",
    "aerie_prover::protocol::pcs::prover::prove_prepared_impl::{{closure}}":
        "prover::prove_prepared_impl::{closure}",
    "<bitz::piop::spartan::falcon512_algebraic::protocol::PreparedFalconAlgebraic>::prove_observed::<<bitz::piop::spartan::falcon512_algebraic::protocol::PreparedFalconAlgebraic>::prove::{closure#0}>":
        "PreparedFalconAlgebraic::prove_observed",
    "core::ptr::drop_in_place::<alloc::vec::Vec<[u64; 2]>>":
        "ptr::drop_in_place",
}
failures = 0
for raw, expected in CASES.items():
    got = short(raw)
    status = "ok " if got == expected else "BAD"
    failures += got != expected
    print(f"{status} {got}")
sys.exit(1 if failures else 0)

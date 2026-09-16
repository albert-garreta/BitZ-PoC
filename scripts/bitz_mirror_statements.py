#!/usr/bin/env python3
"""Mirror the BitZ statement files into the oracle's CLI examples.

`src/bitz/{mul,ecdsa,modr1cs}.rs` here become
`tooling/cli/examples/common/{mul,ecdsa,modr1cs}.rs` there: the tests are
dropped, the crate paths retargeted, and the lift method renamed (`f2z`
here, `bitz` in their circuit trait). Everything else is byte for byte the
same, which is what keeps the two sides' transcripts equal.

    scripts/bitz_mirror_statements.py ~/f2z-benchmark-prime
"""
import sys, pathlib

SPLITMIX = '''/// The probes' seed derivation.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
'''

def mirror(name, needs_splitmix):
    src = pathlib.Path(__file__).resolve().parent.parent / 'src' / 'bitz' / f'{name}.rs'
    text = src.read_text()
    body = text[:text.index('#[cfg(test)]')] if '#[cfg(test)]' in text else text
    body = body.replace('use super::e2e::{CircuitStatement, Error};\n', 'use bitz_cli::end_to_end::{CircuitStatement, Error};\n')
    body = body.replace('use super::statements::splitmix64;\n', SPLITMIX if needs_splitmix else '')
    body = body.replace('cs.f2z_unsigned::<', 'cs.bitz_unsigned::<').replace('cs.f2z::<', 'cs.bitz::<')
    body = body.replace('fused `f2z_unsigned`', 'fused `bitz_unsigned`').replace('`f2z(bit_i)`', '`bitz(bit_i)`')
    if '// MIRROR-DROP-START' in body:
        start = body.index('    // MIRROR-DROP-START')
        end = body.index('    // MIRROR-DROP-END') + len('    // MIRROR-DROP-END\n')
        body = body[:start] + body[end:]
    assert 'super::' not in body and 'crate::' not in body, name
    header = (f'//! Mirrored from f2z-pcs `src/bitz/{name}.rs` by `scripts/bitz_mirror_statements.py`\n'
              f'//! (tests dropped, paths retargeted, `f2z`/`f2z_unsigned` → `bitz`/`bitz_unsigned`): do not edit here.\n//!\n')
    return header + body.rstrip('\n') + '\n'

def main():
    their = pathlib.Path(sys.argv[1]).expanduser()
    out = their / 'tooling' / 'cli' / 'examples' / 'common'
    for name, needs_splitmix in [('mul', True), ('ecdsa', True), ('modr1cs', False)]:
        (out / f'{name}.rs').write_text(mirror(name, needs_splitmix))
        print('mirrored', name)

if __name__ == '__main__':
    main()

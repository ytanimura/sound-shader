# Examples

Examples must be played from root directory.

## pirates

A very cool sound shader by Inigo Quilez.

```bash
cargo run -- examples/pirates.comp
```

## mix

An example for reading several audio file and simple mix.
Overlay and play vocals and instrumentals that are distributed separately.

```bash
cargo run -- -c examples/mix.json
```

## decryption

An example for dealing with the supplied DFT.
It looks simple playing, however, the shader restores the sound from the result of DFT.

```bash
cargo run -- -c examples/decryption.json
```

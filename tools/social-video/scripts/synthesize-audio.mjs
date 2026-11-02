import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

// Original procedural score. No samples, recordings, or third-party music.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const rate = 44100;
const duration = 48;
const beat = 60 / 104;
const count = Math.floor(rate * duration);
const audio = Buffer.alloc(count * 4);
const chords = [[57, 60, 64, 71], [53, 57, 60, 64], [48, 55, 59, 64], [55, 59, 62, 69]];
const melody = [76, 71, 69, 64, 67, 71, 76, 79];
const hz = (midi) => 440 * 2 ** ((midi - 69) / 12);
let seed = 19791014;
const random = () => { seed = (1664525 * seed + 1013904223) >>> 0; return seed / 4294967296 * 2 - 1; };
for (let i = 0; i < count; i++) {
  const t = i / rate;
  const chordIndex = Math.floor(t / (8 * beat));
  const chordT = t % (8 * beat);
  const chord = chords[chordIndex % chords.length];
  const padEnvelope = Math.min(1, chordT / .65) * Math.min(1, (8 * beat - chordT) / .9);
  let left = 0;
  let right = 0;
  for (let v = 0; v < chord.length; v++) {
    const freq = hz(chord[v]);
    const tone = (Math.sin(2 * Math.PI * freq * t) + .35 * Math.sin(2 * Math.PI * freq * 1.003 * t)) * .018 * padEnvelope;
    left += tone * (v % 2 ? .75 : 1);
    right += tone * (v % 2 ? 1 : .75);
  }
  const bt = t % beat;
  const kick = Math.sin(2 * Math.PI * (44 * bt + 100 / 32 * (1 - Math.exp(-32 * bt)))) * Math.exp(-bt * 19) * .11;
  const bass = Math.sin(2 * Math.PI * hz(chord[0] - 12) * t) * Math.exp(-bt * 4.5) * Math.min(1, bt / .012) * .055;
  const hatT = (t + beat / 2) % beat;
  const hat = random() * Math.exp(-hatT * 110) * .012;
  const pluckT = t % (beat * 2);
  const note = melody[Math.floor(t / (beat * 2)) % melody.length];
  const pluck = (Math.sin(2 * Math.PI * hz(note) * pluckT) + .2 * Math.sin(2 * Math.PI * hz(note) * 2 * pluckT)) * Math.exp(-pluckT * 5.5) * Math.min(1, pluckT / .005) * .035;
  const fade = Math.min(1, t / 1.2) * Math.min(1, (duration - t) / 2);
  left = Math.tanh((left + kick + bass + hat + pluck * .6) * fade);
  right = Math.tanh((right + kick + bass + hat * .8 + pluck) * fade);
  audio.writeInt16LE(Math.round(left * 32767), i * 4);
  audio.writeInt16LE(Math.round(right * 32767), i * 4 + 2);
}
const header = Buffer.alloc(44);
header.write('RIFF', 0); header.writeUInt32LE(audio.length + 36, 4); header.write('WAVEfmt ', 8);
header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(2, 22);
header.writeUInt32LE(rate, 24); header.writeUInt32LE(rate * 4, 28); header.writeUInt16LE(4, 32);
header.writeUInt16LE(16, 34); header.write('data', 36); header.writeUInt32LE(audio.length, 40);
fs.mkdirSync(path.join(root, 'public/audio'), {recursive: true});
fs.writeFileSync(path.join(root, 'public/audio/original-score.wav'), Buffer.concat([header, audio]));
console.log('Wrote original 48-second stereo score.');

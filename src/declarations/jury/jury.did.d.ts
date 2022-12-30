import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';

export interface Block {
  'certificate' : Uint8Array,
  'data' : Data,
  'tree' : Uint8Array,
  'previous_hash' : Uint8Array,
}
export interface Data {
  'jurors' : Array<Uint8Array>,
  'jurors_index' : number,
  'kind' : Kind,
  'rand' : [] | [Uint8Array],
}
export type Kind = { 'Add' : null } |
  { 'Remove' : null } |
  { 'Select' : null } |
  { 'Expand' : null };
export interface _SERVICE {
  'add' : ActorMethod<[Array<Uint8Array>], number>,
  'authorize' : ActorMethod<[Principal], undefined>,
  'commit' : ActorMethod<[Uint8Array], [] | [number]>,
  'deauthorize' : ActorMethod<[Principal], undefined>,
  'expand' : ActorMethod<[number, number], number>,
  'find' : ActorMethod<[number, Array<Uint8Array>], Array<[] | [number]>>,
  'get_authorized' : ActorMethod<[], Array<Principal>>,
  'get_block' : ActorMethod<[number], Block>,
  'get_certificate' : ActorMethod<[], [] | [Uint8Array]>,
  'get_jurors' : ActorMethod<[number, number, number], Array<Uint8Array>>,
  'get_pending' : ActorMethod<[], number>,
  'get_size' : ActorMethod<[number], number>,
  'length' : ActorMethod<[], number>,
  'proof' : ActorMethod<[number, Uint32Array], Array<Uint8Array>>,
  'remove' : ActorMethod<[Array<Uint8Array>], number>,
  'select' : ActorMethod<[number, number], number>,
}

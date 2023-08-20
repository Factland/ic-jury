import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';

export interface Block {
  'certificate' : Uint8Array | number[],
  'data' : Data,
  'tree' : Uint8Array | number[],
  'previous_hash' : Uint8Array | number[],
}
export interface Data {
  'jurors' : Array<Uint8Array | number[]>,
  'kind' : Kind,
  'memo' : Uint8Array | number[],
  'rand' : [] | [Uint8Array | number[]],
}
export type Kind = { 'Add' : null } |
  { 'Remove' : null } |
  { 'Select' : null } |
  { 'Expand' : null };
export interface _SERVICE {
  'add' : ActorMethod<
    [Array<Uint8Array | number[]>, Uint8Array | number[]],
    number
  >,
  'authorize' : ActorMethod<[Principal], undefined>,
  'commit' : ActorMethod<[Uint8Array | number[]], [] | [number]>,
  'deauthorize' : ActorMethod<[Principal], undefined>,
  'expand' : ActorMethod<[number, number, Uint8Array | number[]], number>,
  'find' : ActorMethod<
    [number, Array<Uint8Array | number[]>],
    Array<[] | [number]>
  >,
  'get_authorized' : ActorMethod<[], Array<Principal>>,
  'get_block' : ActorMethod<[number], Block>,
  'get_certificate' : ActorMethod<[], [] | [Uint8Array | number[]]>,
  'get_history' : ActorMethod<[Uint8Array | number[]], Uint32Array | number[]>,
  'get_jurors' : ActorMethod<[number], Array<Uint8Array | number[]>>,
  'get_pending' : ActorMethod<[], number>,
  'get_pool' : ActorMethod<
    [number, number, number],
    Array<Uint8Array | number[]>
  >,
  'get_pool_size' : ActorMethod<[number], number>,
  'get_size' : ActorMethod<[number], number>,
  'length' : ActorMethod<[], number>,
  'remove' : ActorMethod<
    [Array<Uint8Array | number[]>, Uint8Array | number[]],
    number
  >,
  'select' : ActorMethod<[number, number, Uint8Array | number[]], number>,
}

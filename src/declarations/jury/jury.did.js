export const idlFactory = ({ IDL }) => {
  const Kind = IDL.Variant({
    'Add' : IDL.Null,
    'Remove' : IDL.Null,
    'Select' : IDL.Null,
    'Expand' : IDL.Null,
  });
  const Data = IDL.Record({
    'jurors' : IDL.Vec(IDL.Vec(IDL.Nat8)),
    'jurors_index' : IDL.Nat32,
    'kind' : Kind,
    'memo' : IDL.Vec(IDL.Nat8),
    'rand' : IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const Block = IDL.Record({
    'certificate' : IDL.Vec(IDL.Nat8),
    'data' : Data,
    'tree' : IDL.Vec(IDL.Nat8),
    'previous_hash' : IDL.Vec(IDL.Nat8),
  });
  return IDL.Service({
    'add' : IDL.Func(
        [IDL.Vec(IDL.Vec(IDL.Nat8)), IDL.Vec(IDL.Nat8)],
        [IDL.Nat32],
        [],
      ),
    'authorize' : IDL.Func([IDL.Principal], [], []),
    'commit' : IDL.Func([IDL.Vec(IDL.Nat8)], [IDL.Opt(IDL.Nat32)], []),
    'deauthorize' : IDL.Func([IDL.Principal], [], []),
    'expand' : IDL.Func(
        [IDL.Nat32, IDL.Nat32, IDL.Vec(IDL.Nat8)],
        [IDL.Nat32],
        [],
      ),
    'find' : IDL.Func(
        [IDL.Nat32, IDL.Vec(IDL.Vec(IDL.Nat8))],
        [IDL.Vec(IDL.Opt(IDL.Nat32))],
        ['query'],
      ),
    'get_authorized' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'get_block' : IDL.Func([IDL.Nat32], [Block], ['query']),
    'get_certificate' : IDL.Func([], [IDL.Opt(IDL.Vec(IDL.Nat8))], ['query']),
    'get_jurors' : IDL.Func(
        [IDL.Nat32],
        [IDL.Vec(IDL.Vec(IDL.Nat8))],
        ['query'],
      ),
    'get_pending' : IDL.Func([], [IDL.Nat32], ['query']),
    'get_pool' : IDL.Func(
        [IDL.Nat32, IDL.Nat32, IDL.Nat32],
        [IDL.Vec(IDL.Vec(IDL.Nat8))],
        ['query'],
      ),
    'get_pool_size' : IDL.Func([IDL.Nat32], [IDL.Nat32], ['query']),
    'get_size' : IDL.Func([IDL.Nat32], [IDL.Nat32], ['query']),
    'length' : IDL.Func([], [IDL.Nat32], ['query']),
    'remove' : IDL.Func(
        [IDL.Vec(IDL.Vec(IDL.Nat8)), IDL.Vec(IDL.Nat8)],
        [IDL.Nat32],
        [],
      ),
    'select' : IDL.Func(
        [IDL.Nat32, IDL.Nat32, IDL.Vec(IDL.Nat8)],
        [IDL.Nat32],
        [],
      ),
  });
};
export const init = ({ IDL }) => { return [IDL.Opt(IDL.Text)]; };

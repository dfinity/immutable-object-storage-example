// AUTO-GENERATED — regenerate with `npm run generate` after deploying.
// This file mirrors rust-backend/src/example_backend.did.

export const idlFactory = ({ IDL }) => {
  const BlobInfo = IDL.Record({
    hash: IDL.Text,
    name: IDL.Text,
    size: IDL.Nat64,
    content_type: IDL.Text,
    created_at: IDL.Nat64,
  });

  const CreateCertificateResult = IDL.Record({
    method: IDL.Text,
    blob_hash: IDL.Text,
  });

  return IDL.Service({
    // Immutable object storage protocol
    _immutableObjectStorageUpdateGatewayPrincipals: IDL.Func([], [], []),
    _immutableObjectStorageBlobIsLive: IDL.Func([IDL.Vec(IDL.Nat8)], [IDL.Bool], ["query"]),
    _immutableObjectStorageBlobsToDelete: IDL.Func([], [IDL.Vec(IDL.Text)], ["query"]),
    _immutableObjectStorageConfirmBlobDeletion: IDL.Func([IDL.Vec(IDL.Vec(IDL.Nat8))], [], []),
    _immutableObjectStorageCreateCertificate: IDL.Func([IDL.Text], [CreateCertificateResult], []),

    // Admin
    add_gateway_principal: IDL.Func([IDL.Principal], [], []),

    // User-facing API
    set_blob_info: IDL.Func([IDL.Text, IDL.Text, IDL.Nat64, IDL.Text], [], []),
    list_blobs: IDL.Func([], [IDL.Vec(BlobInfo)], ["query"]),
    delete_blob: IDL.Func([IDL.Text], [], []),
  });
};

export const init = ({ IDL }) => [];

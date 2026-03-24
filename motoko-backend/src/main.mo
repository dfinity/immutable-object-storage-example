/// Caffeine Object Storage — Example Backend Canister (Motoko)
///
/// This canister does two things:
///
/// 1. Implements the Caffeine storage protocol so the storage gateway can
///    manage blob lifecycle (authorization, liveness checks, deletion
///    confirmation).
///
/// 2. Exposes a simple user-facing API (`listBlobs`, `deleteBlob`,
///    `setBlobInfo`) that the example frontend uses to display and manage
///    stored files.
///
/// ## Integration checklist
///
/// 1. Fetch gateway principals — call
///    `_immutableObjectStorageUpdateGatewayPrincipals` after deployment.
///    This queries the Cashier canister for the current list of authorized
///    gateways. The list is dynamic and may change over time.
/// 2. Upload — your app calls `_immutableObjectStorageCreateCertificate`
///    with a blob hash; the canister records it as live and returns a
///    certificate the client forwards to the gateway.
/// 3. Deletion — mark blobs via `deleteBlob`; the gateway periodically
///    calls `_immutableObjectStorageBlobsToDelete`, deletes the objects,
///    then confirms via `_immutableObjectStorageConfirmBlobDeletion`.

import Array     "mo:core/Array";
import Blob      "mo:core/Blob";
import Iter      "mo:core/Iter";
import Map       "mo:core/Map";
import Nat8      "mo:core/Nat8";
import Principal "mo:core/Principal";
import Runtime   "mo:core/Runtime";
import Set       "mo:core/Set";
import Text      "mo:core/Text";
import Time      "mo:core/Time";

persistent actor class ExampleBackend(initArgs : ?{ cashier_canister_id : ?Principal }) {

    // ── Types ─────────────────────────────────────────────────────────────────

    public type BlobInfo = {
        hash        : Text;
        name        : Text;
        size        : Nat;
        contentType : Text;
        createdAt   : Int;
    };

    public type CreateCertificateResult = {
        method    : Text;
        blob_hash : Text;
    };

    // ── Stable state ──────────────────────────────────────────────────────────
    // mo:core data structures are natively stable — no preupgrade/postupgrade.

    stable let liveBlobs = Map.empty<Text, BlobInfo>();
    stable let pendingDelete = Set.empty<Text>();
    stable let gatewayPrincipals = Set.empty<Principal>();

    let defaultCashier : Principal = Principal.fromText("72ch2-fiaaa-aaaar-qbsvq-cai");

    stable var cashierId : Principal = defaultCashier;

    do {
        switch (initArgs) {
            case (?{ cashier_canister_id = ?id }) { cashierId := id };
            case _ {};
        };
    };

    // ── Internal helpers ──────────────────────────────────────────────────────

    let hexDigits : [Char] = [
        '0', '1', '2', '3', '4', '5', '6', '7',
        '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    func byteToHex(b : Nat8) : Text {
        let n  = Nat8.toNat(b);
        let hi = n / 16;
        let lo = n % 16;
        Text.fromChar(hexDigits[hi]) # Text.fromChar(hexDigits[lo])
    };

    func bytesToHash(bytes : Blob) : ?Text {
        let arr = Blob.toArray(bytes);
        if (arr.size() != 32) return null;
        var hex = "sha256:";
        for (b in arr.values()) {
            hex #= byteToHex(b);
        };
        ?hex
    };

    func callerIsGateway(caller : Principal) : Bool {
        if (Principal.isAnonymous(caller)) return false;
        Set.contains(gatewayPrincipals, Principal.compare, caller)
    };

    // ── _immutableObjectStorage* protocol methods ─────────────────────────────

    /// Fetches the current list of gateway principals from the Cashier canister
    /// and replaces the local authorized set. Call after deployment and
    /// periodically to pick up gateway changes.
    public shared func _immutableObjectStorageUpdateGatewayPrincipals() : async () {
        let cashier : actor { storage_gateway_principal_list_v1 : shared query () -> async [Principal] } =
            actor (Principal.toText(cashierId));
        let principals = await cashier.storage_gateway_principal_list_v1();
        let existing = Iter.toArray(Set.values(gatewayPrincipals));
        for (p in existing.values()) {
            Set.remove(gatewayPrincipals, Principal.compare, p);
        };
        for (p in principals.values()) {
            Set.add(gatewayPrincipals, Principal.compare, p);
        };
    };

    /// Returns whether each blob (identified by a 32-byte hash) is still live.
    /// Input and output arrays have the same length and matching indices.
    public shared query func _immutableObjectStorageBlobsAreLive(hashBytesList : [Blob]) : async [Bool] {
        Array.map<Blob, Bool>(hashBytesList, func(hashBytes : Blob) : Bool {
            switch (bytesToHash(hashBytes)) {
                case null false;
                case (?hash) {
                    Map.containsKey(liveBlobs, Text.compare, hash)
                        and not Set.contains(pendingDelete, Text.compare, hash);
                };
            }
        })
    };

    /// Returns blob hashes marked for deletion (gateway-only).
    public shared query (msg) func _immutableObjectStorageBlobsToDelete() : async [Text] {
        if (not callerIsGateway(msg.caller)) return [];
        Iter.toArray(Set.values(pendingDelete))
    };

    /// Confirms blobs have been deleted from object storage (gateway-only).
    public shared (msg) func _immutableObjectStorageConfirmBlobDeletion(hashBytesList : [Blob]) : async () {
        if (not callerIsGateway(msg.caller)) return;
        for (hashBytes in hashBytesList.values()) {
            switch (bytesToHash(hashBytes)) {
                case null {};
                case (?hash) {
                    Set.remove(pendingDelete, Text.compare, hash);
                    Map.remove(liveBlobs, Text.compare, hash);
                };
            };
        };
    };

    /// Creates an upload certificate. Registers the hash as a live blob.
    public shared func _immutableObjectStorageCreateCertificate(hash : Text) : async CreateCertificateResult {
        if (Text.size(hash) == 0) {
            Runtime.trap("hash must not be empty");
        };
        if (
            not Text.startsWith(hash, #text "sha256:") or
            Text.size(hash) != 71
        ) {
            Runtime.trap("hash must be 'sha256:<64-hex-chars>'");
        };

        Set.remove(pendingDelete, Text.compare, hash);

        if (not Map.containsKey(liveBlobs, Text.compare, hash)) {
            Map.add(liveBlobs, Text.compare, hash, {
                hash        = hash;
                name        = "";
                size        = 0;
                contentType = "";
                createdAt   = Time.now();
            });
        };

        { method = "upload"; blob_hash = hash }
    };

    // ── User-facing API ───────────────────────────────────────────────────────

    /// Attach display metadata to a blob after upload.
    public shared func setBlobInfo(
        hash        : Text,
        name        : Text,
        size        : Nat,
        contentType : Text,
    ) : async () {
        switch (Map.get(liveBlobs, Text.compare, hash)) {
            case null {};
            case (?info) {
                Map.add(liveBlobs, Text.compare, hash, {
                    hash        = info.hash;
                    name        = name;
                    size        = size;
                    contentType = contentType;
                    createdAt   = info.createdAt;
                });
            };
        };
    };

    /// Returns all live blobs, sorted newest-first.
    public shared query func listBlobs() : async [BlobInfo] {
        let all = Array.filter<BlobInfo>(
            Iter.toArray(Map.values(liveBlobs)),
            func(b : BlobInfo) : Bool {
                not Set.contains(pendingDelete, Text.compare, b.hash)
            }
        );
        Array.sort<BlobInfo>(all, func(a, b) {
            if      (a.createdAt > b.createdAt) #less
            else if (a.createdAt < b.createdAt) #greater
            else                                #equal
        })
    };

    /// Mark a blob for deletion.
    public shared func deleteBlob(hash : Text) : async () {
        if (not Map.containsKey(liveBlobs, Text.compare, hash)) {
            Runtime.trap("blob not found");
        };
        Set.add(pendingDelete, Text.compare, hash);
    };
};

/*
 * This file is part of the Trezor project, https://trezor.io/
 *
 * Copyright (C) 2018 Pavol Rusnak <stick@satoshilabs.com>
 *
 * This library is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this library.  If not, see <http://www.gnu.org/licenses/>.
 */

void fsm_msgGetPublicKey(const GetPublicKey *msg) {
  RESP_INIT(PublicKey);

  CHECK_INITIALIZED

  CHECK_PIN

  InputScriptType script_type =
      msg->has_script_type ? msg->script_type : InputScriptType_SPENDADDRESS;

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  const char *curve = coin->curve_name;
  if (msg->has_ecdsa_curve_name) {
    curve = msg->ecdsa_curve_name;
  }

  // Do not allow access to SLIP25 paths.
  if (msg->address_n_count > 0 && msg->address_n[0] == PATH_SLIP25_PURPOSE &&
      config_getSafetyCheckLevel() == SafetyCheckLevel_Strict) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Forbidden key path"));
    layoutHome();
    return;
  }

  // derive m/0' to obtain root_fingerprint
  uint32_t root_fingerprint;
  uint32_t path[1] = {PATH_HARDENED | 0};
  HDNode *node = fsm_getDerivedNode(curve, path, 1, &root_fingerprint);
  if (!node) return;

  uint32_t fingerprint;
  node = fsm_getDerivedNode(curve, msg->address_n, msg->address_n_count,
                            &fingerprint);
  if (!node) return;

  if (hdnode_fill_public_key(node) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Failed to derive public key"));
    layoutHome();
    return;
  }

  resp->node.depth = node->depth;
  resp->node.fingerprint = fingerprint;
  resp->node.child_num = node->child_num;
  resp->node.chain_code.size = 32;
  memcpy(resp->node.chain_code.bytes, node->chain_code, 32);
  resp->node.has_private_key = false;
  resp->node.public_key.size = 33;
  memcpy(resp->node.public_key.bytes, node->public_key, 33);
  if (node->public_key[0] == 1) {
    /* ed25519 public key */
    resp->node.public_key.bytes[0] = 0;
  }

  if (coin->xpub_magic && (script_type == InputScriptType_SPENDADDRESS ||
                           script_type == InputScriptType_SPENDMULTISIG)) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic, resp->xpub,
                            sizeof(resp->xpub));
  } else if (coin->has_segwit &&
             script_type == InputScriptType_SPENDP2SHWITNESS &&
             !msg->ignore_xpub_magic && coin->xpub_magic_segwit_p2sh) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic_segwit_p2sh,
                            resp->xpub, sizeof(resp->xpub));
  } else if (coin->has_segwit &&
             script_type == InputScriptType_SPENDP2SHWITNESS &&
             msg->ignore_xpub_magic && coin->xpub_magic) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic, resp->xpub,
                            sizeof(resp->xpub));
  } else if (coin->has_segwit && script_type == InputScriptType_SPENDWITNESS &&
             !msg->ignore_xpub_magic && coin->xpub_magic_segwit_native) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic_segwit_native,
                            resp->xpub, sizeof(resp->xpub));
  } else if (coin->has_segwit && script_type == InputScriptType_SPENDWITNESS &&
             msg->ignore_xpub_magic && coin->xpub_magic) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic, resp->xpub,
                            sizeof(resp->xpub));
  } else if (coin->has_taproot && script_type == InputScriptType_SPENDTAPROOT &&
             coin->xpub_magic) {
    hdnode_serialize_public(node, fingerprint, coin->xpub_magic, resp->xpub,
                            sizeof(resp->xpub));
  } else {
    fsm_sendFailure(FailureType_Failure_DataError,
                    _("Invalid combination of coin and script_type"));
    layoutHome();
    return;
  }

  if (msg->has_show_display && msg->show_display) {
    for (int page = 0; page < 2; page++) {
      layoutXPUB(resp->xpub, page);
      if (!protectButton(ButtonRequestType_ButtonRequest_PublicKey, true)) {
        memzero(resp, sizeof(PublicKey));
        fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
        layoutHome();
        return;
      }
    }
  }

  resp->has_root_fingerprint = true;
  resp->root_fingerprint = root_fingerprint;

  msg_write(MessageType_MessageType_PublicKey, resp);
  layoutHome();
}

void fsm_msgSignTx(const SignTx *msg) {
  CHECK_INITIALIZED

  CHECK_PARAM(msg->inputs_count > 0,
              _("Transaction must have at least one input"));
  CHECK_PARAM(msg->outputs_count > 0,
              _("Transaction must have at least one output"));
  CHECK_PARAM(msg->inputs_count + msg->outputs_count >= msg->inputs_count,
              _("Value overflow"));

  CHECK_PIN

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  CHECK_PARAM((coin->decred || coin->overwintered) || !msg->has_expiry,
              _("Expiry not enabled on this coin."))
  CHECK_PARAM(coin->timestamp || !msg->has_timestamp,
              _("Timestamp not enabled on this coin."))
  CHECK_PARAM(!coin->timestamp || msg->timestamp, _("Timestamp must be set."))

  const HDNode *node = fsm_getDerivedNode(coin->curve_name, NULL, 0, NULL);
  if (!node) return;

  signing_init(msg, coin, node);
}

void fsm_msgTxAck(TxAck *msg) {
  CHECK_UNLOCKED

  CHECK_PARAM(msg->has_tx, _("No transaction provided"));

  signing_txack(&(msg->tx));
}

bool fsm_checkCoinPath(const CoinInfo *coin, InputScriptType script_type,
                       uint32_t address_n_count, const uint32_t *address_n,
                       bool has_multisig, bool show_warning) {
  if (coin_path_check(coin, script_type, address_n_count, address_n,
                      has_multisig, true)) {
    return true;
  }

  if (config_getSafetyCheckLevel() == SafetyCheckLevel_Strict &&
      !coin_path_check(coin, script_type, address_n_count, address_n,
                       has_multisig, false)) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Forbidden key path"));
    return false;
  }

  if (show_warning) {
    return fsm_layoutPathWarning();
  }

  return true;
}

bool fsm_checkScriptType(const CoinInfo *coin, InputScriptType script_type) {
  if (!is_internal_input_script_type(script_type)) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Invalid script type"));
    return false;
  }

  if (is_segwit_input_script_type(script_type) && !coin->has_segwit) {
    fsm_sendFailure(FailureType_Failure_DataError,
                    _("Segwit not enabled on this coin"));
    return false;
  }

  if (script_type == InputScriptType_SPENDTAPROOT && !coin->has_taproot) {
    fsm_sendFailure(FailureType_Failure_DataError,
                    _("Taproot not enabled on this coin"));
    return false;
  }

  return true;
}

void fsm_msgGetAddress(const GetAddress *msg) {
  RESP_INIT(Address);

  CHECK_INITIALIZED

  CHECK_PIN

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  if (!fsm_checkCoinPath(coin, msg->script_type, msg->address_n_count,
                         msg->address_n, msg->has_multisig,
                         msg->show_display)) {
    layoutHome();
    return;
  }

  HDNode *node = fsm_getDerivedNode(coin->curve_name, msg->address_n,
                                    msg->address_n_count, NULL);
  if (!node) return;

  if (hdnode_fill_public_key(node) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Failed to derive public key"));
    layoutHome();
    return;
  }

  char address[MAX_ADDR_SIZE];
  if (msg->has_multisig) {  // use progress bar only for multisig
    layoutProgress(_("Computing address"), 0);
  }
  if (!compute_address(coin, msg->script_type, node, msg->has_multisig,
                       &msg->multisig, address)) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Can't encode address"));
    layoutHome();
    return;
  }

  if (msg->has_show_display && msg->show_display) {
    char desc[20] = {0};
    int multisig_index = 0;
    if (msg->has_multisig) {
      strlcpy(desc, "Multisig __ of __:", sizeof(desc));
      const uint32_t m = msg->multisig.m;
      const uint32_t n = cryptoMultisigPubkeyCount(&(msg->multisig));
      desc[9] = (m < 10) ? ' ' : ('0' + (m / 10));
      desc[10] = '0' + (m % 10);
      desc[15] = (n < 10) ? ' ' : ('0' + (n / 10));
      desc[16] = '0' + (n % 10);
      multisig_index =
          cryptoMultisigPubkeyIndex(coin, &(msg->multisig), node->public_key);
    } else {
      strlcpy(desc, _("Address:"), sizeof(desc));
    }

    uint32_t multisig_xpub_magic = coin->xpub_magic;
    if (msg->has_multisig && coin->has_segwit) {
      if (!msg->has_ignore_xpub_magic || !msg->ignore_xpub_magic) {
        if (msg->script_type == InputScriptType_SPENDWITNESS &&
            coin->xpub_magic_segwit_native) {
          multisig_xpub_magic = coin->xpub_magic_segwit_native;
        } else if (msg->script_type == InputScriptType_SPENDP2SHWITNESS &&
                   coin->xpub_magic_segwit_p2sh) {
          multisig_xpub_magic = coin->xpub_magic_segwit_p2sh;
        }
      }
    }

    bool is_cashaddr = coin->cashaddr_prefix != NULL;
    if (!fsm_layoutAddress(address, desc, is_cashaddr,
                           is_cashaddr ? strlen(coin->cashaddr_prefix) + 1 : 0,
                           msg->address_n, msg->address_n_count, false,
                           msg->has_multisig ? &(msg->multisig) : NULL,
                           multisig_index, multisig_xpub_magic, coin)) {
      return;
    }
  }

  strlcpy(resp->address, address, sizeof(resp->address));
  msg_write(MessageType_MessageType_Address, resp);
  layoutHome();
}

void fsm_msgSignMessage(const SignMessage *msg) {
  // CHECK_PARAM(is_ascii_only(msg->message.bytes, msg->message.size), _("Cannot
  // sign non-ASCII strings"));

  RESP_INIT(MessageSignature);

  CHECK_INITIALIZED

  CHECK_PIN

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  if (!fsm_checkCoinPath(coin, msg->script_type, msg->address_n_count,
                         msg->address_n, false, true)) {
    layoutHome();
    return;
  }

  HDNode *node = fsm_getDerivedNode(coin->curve_name, msg->address_n,
                                    msg->address_n_count, NULL);
  if (!node) return;

  if (hdnode_fill_public_key(node) != 0) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Failed to derive public key"));
    layoutHome();
    return;
  }

  if (!compute_address(coin, msg->script_type, node, false, NULL,
                       resp->address)) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Error computing address"));
    layoutHome();
    return;
  }

  layoutVerifyAddress(coin, resp->address);
  if (!protectButton(ButtonRequestType_ButtonRequest_Other, false)) {
    fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
    layoutHome();
    return;
  }

  if (!fsm_layoutSignMessage(msg->message.bytes, msg->message.size)) {
    fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
    layoutHome();
    return;
  }

  layoutProgressSwipe(_("Signing"), 0);
  if (cryptoMessageSign(coin, node, msg->script_type, msg->no_script_type,
                        msg->message.bytes, msg->message.size,
                        resp->signature.bytes) == 0) {
    resp->signature.size = 65;
    msg_write(MessageType_MessageType_MessageSignature, resp);
  } else {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Error signing message"));
  }
  layoutHome();
}

void fsm_msgVerifyMessage(const VerifyMessage *msg) {
  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;
  layoutProgressSwipe(_("Verifying"), 0);
  if (msg->signature.size != 65) {
    fsm_sendFailure(FailureType_Failure_ProcessError, _("Invalid signature"));
    layoutHome();
    return;
  }

  int result = cryptoMessageVerify(coin, msg->message.bytes, msg->message.size,
                                   msg->address, msg->signature.bytes);
  if (result == 0) {
    layoutVerifyAddress(coin, msg->address);
    if (!protectButton(ButtonRequestType_ButtonRequest_Other, false)) {
      fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
      layoutHome();
      return;
    }

    if (!fsm_layoutVerifyMessage(msg->message.bytes, msg->message.size)) {
      fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
      layoutHome();
      return;
    }

    layoutDialogSwipe(&bmp_icon_ok, NULL, _("Continue"), NULL, NULL,
                      _("The signature is valid."), NULL, NULL, NULL, NULL);
    if (!protectButton(ButtonRequestType_ButtonRequest_Other, true)) {
      fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
      layoutHome();
      return;
    }

    fsm_sendSuccess(_("Message verified"));
  } else if (result == 1) {
    fsm_sendFailure(FailureType_Failure_DataError, _("Invalid address"));
  } else {
    fsm_sendFailure(FailureType_Failure_ProcessError, _("Invalid signature"));
  }
  layoutHome();
}

bool fsm_getOwnershipId(uint8_t *script_pubkey, size_t script_pubkey_size,
                        uint8_t ownership_id[32]) {
  const char *OWNERSHIP_ID_KEY_PATH[] = {"SLIP-0019",
                                         "Ownership identification key"};

  uint8_t ownership_id_key[32] = {0};
  if (!fsm_getSlip21Key(OWNERSHIP_ID_KEY_PATH, 2, ownership_id_key)) {
    return false;
  }

  hmac_sha256(ownership_id_key, sizeof(ownership_id_key), script_pubkey,
              script_pubkey_size, ownership_id);

  return true;
}

void fsm_msgGetOwnershipId(const GetOwnershipId *msg) {
  RESP_INIT(OwnershipId);

  CHECK_INITIALIZED

  CHECK_PIN

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  if (!fsm_checkCoinPath(coin, msg->script_type, msg->address_n_count,
                         msg->address_n, msg->has_multisig, false)) {
    layoutHome();
    return;
  }

  if (!fsm_checkScriptType(coin, msg->script_type)) {
    layoutHome();
    return;
  }

  HDNode *node = fsm_getDerivedNode(coin->curve_name, msg->address_n,
                                    msg->address_n_count, NULL);
  if (!node) return;

  uint8_t script_pubkey[520] = {0};
  pb_size_t script_pubkey_size = 0;
  if (!get_script_pubkey(coin, node, msg->has_multisig, &msg->multisig,
                         msg->script_type, script_pubkey,
                         &script_pubkey_size)) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Failed to derive scriptPubKey"));
    layoutHome();
    return;
  }

  if (!fsm_getOwnershipId(script_pubkey, script_pubkey_size,
                          resp->ownership_id.bytes)) {
    return;
  }

  resp->ownership_id.size = 32;

  msg_write(MessageType_MessageType_OwnershipId, resp);
  layoutHome();
}

void fsm_msgGetOwnershipProof(const GetOwnershipProof *msg) {
  RESP_INIT(OwnershipProof);

  CHECK_INITIALIZED

  CHECK_PIN

  if (msg->has_multisig) {
    // The legacy implementation currently only supports singlesig native segwit
    // v0 and v1, the bare minimum for CoinJoin.
    fsm_sendFailure(FailureType_Failure_DataError,
                    _("Multisig not supported."));
    layoutHome();
    return;
  }

  const CoinInfo *coin = fsm_getCoin(msg->has_coin_name, msg->coin_name);
  if (!coin) return;

  if (!fsm_checkCoinPath(coin, msg->script_type, msg->address_n_count,
                         msg->address_n, msg->has_multisig, false)) {
    layoutHome();
    return;
  }

  if (!fsm_checkScriptType(coin, msg->script_type)) {
    layoutHome();
    return;
  }

  HDNode *node = fsm_getDerivedNode(coin->curve_name, msg->address_n,
                                    msg->address_n_count, NULL);
  if (!node) return;

  uint8_t script_pubkey[520] = {0};
  pb_size_t script_pubkey_size = 0;
  if (!get_script_pubkey(coin, node, msg->has_multisig, &msg->multisig,
                         msg->script_type, script_pubkey,
                         &script_pubkey_size)) {
    fsm_sendFailure(FailureType_Failure_ProcessError,
                    _("Failed to derive scriptPubKey"));
    layoutHome();
    return;
  }

  uint8_t ownership_id[32] = {0};
  if (!fsm_getOwnershipId(script_pubkey, script_pubkey_size, ownership_id)) {
    return;
  }

  // Providing an ownership ID is optional in case of singlesig, but if one is
  // provided, then it should match.
  if (msg->ownership_ids_count) {
    if (msg->ownership_ids_count != 1 ||
        msg->ownership_ids[0].size != sizeof(ownership_id) ||
        memcmp(ownership_id, msg->ownership_ids[0].bytes,
               sizeof(ownership_id) != 0)) {
      fsm_sendFailure(FailureType_Failure_DataError,
                      _("Invalid ownership identifier"));
      layoutHome();
      return;
    }
  }

  // In order to set the "user confirmation" bit in the proof, the user must
  // actually confirm.
  uint8_t flags = 0;
  if (msg->user_confirmation) {
    flags |= 1;
    layoutConfirmOwnershipProof();
    if (!protectButton(ButtonRequestType_ButtonRequest_ProtectCall, false)) {
      fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
      layoutHome();
      return;
    }

    if (msg->has_commitment_data) {
      if (!fsm_layoutCommitmentData(msg->commitment_data.bytes,
                                    msg->commitment_data.size)) {
        fsm_sendFailure(FailureType_Failure_ActionCancelled, NULL);
        layoutHome();
        return;
      }
    }
  }

  size_t r = 0;
  memcpy(resp->ownership_proof.bytes + r, "SL\x00\x19", 4);
  r += 4;
  resp->ownership_proof.bytes[r] = flags;
  r += 1;
  r += ser_length(1, resp->ownership_proof.bytes + r);
  memcpy(resp->ownership_proof.bytes + r, ownership_id, sizeof(ownership_id));
  r += sizeof(ownership_id);

  Hasher hasher = {0};
  hasher_InitParam(&hasher, HASHER_SHA2, NULL, 0);
  hasher_Update(&hasher, resp->ownership_proof.bytes, r);
  tx_script_hash(&hasher, script_pubkey_size, script_pubkey);
  tx_script_hash(&hasher, msg->commitment_data.size,
                 msg->commitment_data.bytes);
  uint8_t digest[SHA256_DIGEST_LENGTH] = {0};
  hasher_Final(&hasher, digest);

  if (msg->script_type == InputScriptType_SPENDWITNESS) {
    if (!tx_sign_ecdsa(coin->curve->params, node->private_key, digest,
                       resp->signature.bytes, &resp->signature.size)) {
      fsm_sendFailure(FailureType_Failure_ProcessError, _("Signing failed"));
      layoutHome();
      return;
    }
    r += ser_length(0, resp->ownership_proof.bytes + r);  // Empty scriptSig.
    r += serialize_p2wpkh_witness(resp->signature.bytes, resp->signature.size,
                                  node->public_key, 33, SIGHASH_ALL,
                                  resp->ownership_proof.bytes + r);
  } else if (msg->script_type == InputScriptType_SPENDTAPROOT) {
    if (!tx_sign_bip340(node->private_key, digest, resp->signature.bytes,
                        &resp->signature.size)) {
      fsm_sendFailure(FailureType_Failure_ProcessError, _("Signing failed"));
      layoutHome();
      return;
    }
    r += ser_length(0, resp->ownership_proof.bytes + r);  // Empty scriptSig.
    r += serialize_p2tr_witness(resp->signature.bytes, resp->signature.size, 0,
                                resp->ownership_proof.bytes + r);
  } else {
    fsm_sendFailure(FailureType_Failure_DataError,
                    _("Unsupported script type."));
    layoutHome();
    return;
  }

  resp->ownership_proof.size = r;
  msg_write(MessageType_MessageType_OwnershipProof, resp);
  layoutHome();
}

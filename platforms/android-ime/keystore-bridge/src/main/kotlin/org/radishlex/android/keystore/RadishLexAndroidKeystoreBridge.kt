package org.radishlex.android.keystore

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.UserNotAuthenticatedException
import java.security.GeneralSecurityException
import java.security.InvalidAlgorithmParameterException
import java.security.InvalidKeyException
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.KeyStoreException
import java.security.NoSuchAlgorithmException
import java.security.NoSuchProviderException
import java.security.PrivateKey
import java.security.PublicKey
import java.security.ProviderException
import java.security.Signature
import java.security.UnrecoverableEntryException

object RadishLexAndroidKeystoreBridgeContract {
    const val CONTRACT_VERSION: Int = 1
    const val PROVIDER: String = "AndroidKeyStore"
    const val SIGNATURE_ALGORITHM: String = "Ed25519"

    const val ERROR_STORAGE_BACKEND_UNAVAILABLE: String = "storage_backend_unavailable"
    const val ERROR_UNSUPPORTED_SIGNATURE_ALGORITHM: String = "unsupported_signature_algorithm"
    const val ERROR_UNSUPPORTED_STORAGE_BACKEND: String = "unsupported_storage_backend"
    const val ERROR_PRIVATE_KEY_UNAVAILABLE: String = "private_key_unavailable"
    const val ERROR_PRIVATE_KEY_LOCKED: String = "private_key_locked"
    const val ERROR_PRIVATE_KEY_ACCESS_DENIED: String = "private_key_access_denied"
    const val ERROR_PRIVATE_KEY_USER_PRESENCE_REQUIRED: String = "private_key_user_presence_required"
    const val ERROR_PRIVATE_KEY_CORRUPTED: String = "private_key_corrupted"

    const val RAW_ED25519_PUBLIC_KEY_SIZE: Int = 32
    const val ED25519_SIGNATURE_SIZE: Int = 64
}

enum class RadishLexAndroidKeystoreOperation(val wireName: String) {
    CreateSigningKey("create_signing_key"),
    LoadPublicKey("load_public_key"),
    Sign("sign"),
    DeleteSigningKey("delete_signing_key");

    companion object {
        fun parse(value: String): RadishLexAndroidKeystoreOperation? {
            return values().firstOrNull { it.wireName == value }
        }
    }
}

class RadishLexAndroidKeystoreBridgeRequest(
    val contractVersion: Int,
    val operation: String,
    val signingKeyId: String,
    val alias: String,
    canonicalBytes: ByteArray = ByteArray(0)
) {
    val canonicalBytes: ByteArray = canonicalBytes.copyOf()

    fun validate(expectedOperation: RadishLexAndroidKeystoreOperation): String? {
        if (contractVersion != RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION) {
            return RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
        }
        if (RadishLexAndroidKeystoreOperation.parse(operation) != expectedOperation) {
            return RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
        }
        if (signingKeyId.isBlank() || alias.isBlank()) {
            return RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
        }
        if (expectedOperation == RadishLexAndroidKeystoreOperation.Sign) {
            if (canonicalBytes.isEmpty()) {
                return RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
            }
        } else if (canonicalBytes.isNotEmpty()) {
            return RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
        }
        return null
    }

    override fun toString(): String {
        return "RadishLexAndroidKeystoreBridgeRequest(" +
            "contractVersion=$contractVersion, " +
            "operation=$operation, " +
            "signingKeyId=$signingKeyId, " +
            "aliasLen=${alias.length}, " +
            "canonicalBytesLen=${canonicalBytes.size}" +
            ")"
    }
}

class RadishLexAndroidKeystoreBridgeResult private constructor(
    publicKey: ByteArray,
    signature: ByteArray,
    val errorCode: String?
) {
    val publicKey: ByteArray = publicKey.copyOf()
    val signature: ByteArray = signature.copyOf()
    val isSuccess: Boolean = errorCode == null

    override fun toString(): String {
        return "RadishLexAndroidKeystoreBridgeResult(" +
            "publicKeyLen=${publicKey.size}, " +
            "signatureLen=${signature.size}, " +
            "errorCode=$errorCode" +
            ")"
    }

    companion object {
        fun publicKey(publicKey: ByteArray): RadishLexAndroidKeystoreBridgeResult {
            return RadishLexAndroidKeystoreBridgeResult(publicKey, ByteArray(0), null)
        }

        fun signature(signature: ByteArray): RadishLexAndroidKeystoreBridgeResult {
            return RadishLexAndroidKeystoreBridgeResult(ByteArray(0), signature, null)
        }

        fun emptySuccess(): RadishLexAndroidKeystoreBridgeResult {
            return RadishLexAndroidKeystoreBridgeResult(ByteArray(0), ByteArray(0), null)
        }

        fun failure(errorCode: String): RadishLexAndroidKeystoreBridgeResult {
            return RadishLexAndroidKeystoreBridgeResult(ByteArray(0), ByteArray(0), errorCode)
        }
    }
}

class RadishLexAndroidKeystoreBridge(
    private val provider: String = RadishLexAndroidKeystoreBridgeContract.PROVIDER,
    private val signatureAlgorithm: String = RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM
) {
    fun createSigningKey(
        request: RadishLexAndroidKeystoreBridgeRequest
    ): RadishLexAndroidKeystoreBridgeResult {
        request.validate(RadishLexAndroidKeystoreOperation.CreateSigningKey)?.let {
            return RadishLexAndroidKeystoreBridgeResult.failure(it)
        }
        return try {
            val keyPairGenerator = KeyPairGenerator.getInstance(signatureAlgorithm, provider)
            val spec = KeyGenParameterSpec.Builder(
                request.alias,
                KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
            ).build()
            keyPairGenerator.initialize(spec)
            keyPairGenerator.generateKeyPair()
            val certificate = openKeyStore().getCertificate(request.alias)
                ?: return RadishLexAndroidKeystoreBridgeResult.failure(
                    RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_UNAVAILABLE
                )
            publicKeyResult(certificate.publicKey)
        } catch (error: Exception) {
            RadishLexAndroidKeystoreBridgeResult.failure(mapError(error))
        }
    }

    fun loadPublicKey(
        request: RadishLexAndroidKeystoreBridgeRequest
    ): RadishLexAndroidKeystoreBridgeResult {
        request.validate(RadishLexAndroidKeystoreOperation.LoadPublicKey)?.let {
            return RadishLexAndroidKeystoreBridgeResult.failure(it)
        }
        return try {
            val certificate = openKeyStore().getCertificate(request.alias)
                ?: return RadishLexAndroidKeystoreBridgeResult.failure(
                    RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_UNAVAILABLE
                )
            publicKeyResult(certificate.publicKey)
        } catch (error: Exception) {
            RadishLexAndroidKeystoreBridgeResult.failure(mapError(error))
        }
    }

    fun sign(request: RadishLexAndroidKeystoreBridgeRequest): RadishLexAndroidKeystoreBridgeResult {
        request.validate(RadishLexAndroidKeystoreOperation.Sign)?.let {
            return RadishLexAndroidKeystoreBridgeResult.failure(it)
        }
        return try {
            val privateKey = loadPrivateKey(request.alias)
                ?: return RadishLexAndroidKeystoreBridgeResult.failure(
                    RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_UNAVAILABLE
                )
            val signer = Signature.getInstance(signatureAlgorithm)
            signer.initSign(privateKey)
            signer.update(request.canonicalBytes)
            val signature = signer.sign()
            if (signature.size != RadishLexAndroidKeystoreBridgeContract.ED25519_SIGNATURE_SIZE) {
                return RadishLexAndroidKeystoreBridgeResult.failure(
                    RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
                )
            }
            RadishLexAndroidKeystoreBridgeResult.signature(signature)
        } catch (error: Exception) {
            RadishLexAndroidKeystoreBridgeResult.failure(mapError(error))
        }
    }

    fun deleteSigningKey(
        request: RadishLexAndroidKeystoreBridgeRequest
    ): RadishLexAndroidKeystoreBridgeResult {
        request.validate(RadishLexAndroidKeystoreOperation.DeleteSigningKey)?.let {
            return RadishLexAndroidKeystoreBridgeResult.failure(it)
        }
        return try {
            openKeyStore().deleteEntry(request.alias)
            RadishLexAndroidKeystoreBridgeResult.emptySuccess()
        } catch (error: Exception) {
            RadishLexAndroidKeystoreBridgeResult.failure(mapError(error))
        }
    }

    private fun openKeyStore(): KeyStore {
        return KeyStore.getInstance(provider).apply { load(null) }
    }

    private fun loadPrivateKey(alias: String): PrivateKey? {
        val entry = openKeyStore().getEntry(alias, null)
        return (entry as? KeyStore.PrivateKeyEntry)?.privateKey
    }

    private fun publicKeyResult(publicKey: PublicKey): RadishLexAndroidKeystoreBridgeResult {
        val rawPublicKey = rawEd25519PublicKey(publicKey.encoded)
        if (rawPublicKey == null) {
            val errorCode = if (isRequestedPublicKeyAlgorithm(publicKey)) {
                RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
            } else {
                RadishLexAndroidKeystoreBridgeContract.ERROR_UNSUPPORTED_SIGNATURE_ALGORITHM
            }
            return RadishLexAndroidKeystoreBridgeResult.failure(errorCode)
        }
        return RadishLexAndroidKeystoreBridgeResult.publicKey(rawPublicKey)
    }

    private fun isRequestedPublicKeyAlgorithm(publicKey: PublicKey): Boolean {
        val algorithm = publicKey.algorithm
        return algorithm.equals(signatureAlgorithm, ignoreCase = true) ||
            algorithm.equals("EdDSA", ignoreCase = true)
    }

    private fun rawEd25519PublicKey(encodedPublicKey: ByteArray?): ByteArray? {
        if (encodedPublicKey == null) {
            return null
        }
        val prefix = ED25519_SPKI_PREFIXES.firstOrNull {
            encodedPublicKey.size == it.size +
                RadishLexAndroidKeystoreBridgeContract.RAW_ED25519_PUBLIC_KEY_SIZE &&
                hasPrefix(encodedPublicKey, it)
        } ?: return null
        return encodedPublicKey.copyOfRange(
            prefix.size,
            encodedPublicKey.size
        )
    }

    private fun hasPrefix(bytes: ByteArray, prefix: ByteArray): Boolean {
        for (index in prefix.indices) {
            if (bytes[index] != prefix[index]) {
                return false
            }
        }
        return true
    }

    private fun mapError(error: Exception): String {
        return when (error) {
            is NoSuchAlgorithmException -> RadishLexAndroidKeystoreBridgeContract.ERROR_UNSUPPORTED_SIGNATURE_ALGORITHM
            is InvalidAlgorithmParameterException -> RadishLexAndroidKeystoreBridgeContract.ERROR_UNSUPPORTED_SIGNATURE_ALGORITHM
            is NoSuchProviderException -> RadishLexAndroidKeystoreBridgeContract.ERROR_UNSUPPORTED_STORAGE_BACKEND
            is KeyPermanentlyInvalidatedException -> RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_ACCESS_DENIED
            is UserNotAuthenticatedException -> RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_USER_PRESENCE_REQUIRED
            is UnrecoverableEntryException -> RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_UNAVAILABLE
            is InvalidKeyException -> RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_CORRUPTED
            is KeyStoreException -> RadishLexAndroidKeystoreBridgeContract.ERROR_STORAGE_BACKEND_UNAVAILABLE
            is ProviderException -> RadishLexAndroidKeystoreBridgeContract.ERROR_STORAGE_BACKEND_UNAVAILABLE
            is GeneralSecurityException -> RadishLexAndroidKeystoreBridgeContract.ERROR_STORAGE_BACKEND_UNAVAILABLE
            else -> RadishLexAndroidKeystoreBridgeContract.ERROR_STORAGE_BACKEND_UNAVAILABLE
        }
    }

    companion object {
        private val ED25519_SPKI_PREFIX = byteArrayOf(
            0x30,
            0x2a,
            0x30,
            0x05,
            0x06,
            0x03,
            0x2b,
            0x65,
            0x70,
            0x03,
            0x21,
            0x00
        )
        private val ED25519_SPKI_PREFIX_WITH_NULL_PARAMS = byteArrayOf(
            0x30,
            0x2c,
            0x30,
            0x07,
            0x06,
            0x03,
            0x2b,
            0x65,
            0x70,
            0x05,
            0x00,
            0x03,
            0x21,
            0x00
        )
        private val ED25519_SPKI_PREFIXES = arrayOf(
            ED25519_SPKI_PREFIX,
            ED25519_SPKI_PREFIX_WITH_NULL_PARAMS
        )
    }
}

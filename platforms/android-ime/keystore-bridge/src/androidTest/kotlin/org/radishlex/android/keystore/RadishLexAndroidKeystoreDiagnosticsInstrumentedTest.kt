package org.radishlex.android.keystore

import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.Security
import java.security.Signature
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class RadishLexAndroidKeystoreDiagnosticsInstrumentedTest {
    @Test
    fun reportAndroidKeystoreProviderBehavior() {
        val arguments = InstrumentationRegistry.getArguments()
        assumeTrue(
            "Set $ARG_RUN_DIAGNOSTICS=true only after approving Android Keystore diagnostics.",
            arguments.getString(ARG_RUN_DIAGNOSTICS) == "true"
        )

        val providerAlias = "org.radishlex.sync.signing.instrumented.diagnostics.provider"
        val bridgeAlias = "org.radishlex.sync.signing.instrumented.diagnostics.bridge"
        deleteAlias(providerAlias)
        deleteAlias(bridgeAlias)

        try {
            emit("schema=android-keystore-diagnostics-v1")
            emitDeviceMetadata()
            emitSignatureFactoryMetadata()
            emitProviderKeyPairMetadata(providerAlias)
            emitBridgeMetadata(bridgeAlias)
        } finally {
            emit("cleanup.provider=${deleteAlias(providerAlias)}")
            emit("cleanup.bridge=${deleteAlias(bridgeAlias)}")
        }
    }

    private fun emitDeviceMetadata() {
        emit("android.release=${Build.VERSION.RELEASE}")
        emit("android.api=${Build.VERSION.SDK_INT}")
        emit("android.security_patch=${Build.VERSION.SECURITY_PATCH}")
        emit("device.manufacturer=${Build.MANUFACTURER}")
        emit("device.model=${Build.MODEL}")
    }

    private fun emitSignatureFactoryMetadata() {
        val provider = Security.getProvider(RadishLexAndroidKeystoreBridgeContract.PROVIDER)
        emit("keystore_provider.available=${provider != null}")
        emit("keystore_provider.name=${provider?.name ?: VALUE_UNAVAILABLE}")
        emit("signature_factory=${signatureFactoryResult()}")
    }

    private fun emitProviderKeyPairMetadata(alias: String) {
        val keyPairGenerator = try {
            KeyPairGenerator.getInstance(
                RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM,
                RadishLexAndroidKeystoreBridgeContract.PROVIDER
            )
        } catch (error: Exception) {
            emit("keypair_generator=error:${error.javaClass.simpleName}")
            return
        }
        emit(
            "keypair_generator=success;" +
                "algorithm=${keyPairGenerator.algorithm};" +
                "provider=${keyPairGenerator.provider.name}"
        )

        try {
            val spec = KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
            ).build()
            keyPairGenerator.initialize(spec)
            keyPairGenerator.generateKeyPair()
        } catch (error: Exception) {
            emit("keypair_generate=error:${error.javaClass.simpleName}")
            return
        }
        emit("keypair_generate=success")

        val entry = privateKeyEntry(alias)
        val publicKey = openKeyStore().getCertificate(alias)?.publicKey
        val encodedPublicKey = publicKey?.encoded
        emit(
            "generated_public_key=" +
                "algorithm=${publicKey?.algorithm ?: VALUE_UNAVAILABLE};" +
                "format=${publicKey?.format ?: VALUE_UNAVAILABLE};" +
                "encodedLen=${encodedPublicKey?.size ?: 0};" +
                "encodedHead=${encodedHead(encodedPublicKey)}"
        )
        emit("generated_private_key_algorithm=${entry?.privateKey?.algorithm ?: VALUE_UNAVAILABLE}")
        emit("generated_key_info=${keyInfoResult(entry?.privateKey)}")
        emit("ed25519_sign_with_generated_key=${signResult(entry?.privateKey)}")
    }

    private fun emitBridgeMetadata(alias: String) {
        val signingKeyId = "radishlex-instrumented-diagnostics"
        val bridge = RadishLexAndroidKeystoreBridge()
        val create = bridge.createSigningKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.CreateSigningKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
        emit(
            "bridge_create=" + if (create.isSuccess) {
                "success;publicKeyLen=${create.publicKey.size}"
            } else {
                "error:${create.errorCode}"
            }
        )

        val loaded = bridge.loadPublicKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.LoadPublicKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
        emit(
            "bridge_load=" + if (loaded.isSuccess) {
                "success;publicKeyLen=${loaded.publicKey.size}"
            } else {
                "error:${loaded.errorCode}"
            }
        )
    }

    private fun signatureFactoryResult(): String {
        return try {
            val signature = Signature.getInstance(
                RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM
            )
            "success;algorithm=${signature.algorithm};provider=${signature.provider.name}"
        } catch (error: Exception) {
            "error:${error.javaClass.simpleName}"
        }
    }

    private fun signResult(privateKey: PrivateKey?): String {
        if (privateKey == null) {
            return "not_available"
        }
        return try {
            val signer = Signature.getInstance(
                RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM
            )
            signer.initSign(privateKey)
            signer.update(DIAGNOSTIC_CANONICAL_BYTES)
            val signature = signer.sign()
            "success;signatureLen=${signature.size}"
        } catch (error: Exception) {
            "error:${error.javaClass.simpleName}"
        }
    }

    private fun keyInfoResult(privateKey: PrivateKey?): String {
        if (privateKey == null) {
            return "not_available"
        }
        return try {
            val keyInfo = KeyFactory.getInstance(
                privateKey.algorithm,
                RadishLexAndroidKeystoreBridgeContract.PROVIDER
            ).getKeySpec(privateKey, KeyInfo::class.java)
            "success;" +
                "insideSecureHardware=${insideSecureHardware(keyInfo)};" +
                "securityLevel=${securityLevel(keyInfo)};" +
                "userAuthenticationRequired=${keyInfo.isUserAuthenticationRequired}"
        } catch (error: Exception) {
            "error:${error.javaClass.simpleName}"
        }
    }

    private fun insideSecureHardware(keyInfo: KeyInfo): String {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (
                keyInfo.securityLevel == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT ||
                    keyInfo.securityLevel == KeyProperties.SECURITY_LEVEL_STRONGBOX
                ).toString()
        } else {
            legacyInsideSecureHardware(keyInfo).toString()
        }
    }

    @Suppress("DEPRECATION")
    private fun legacyInsideSecureHardware(keyInfo: KeyInfo): Boolean {
        return keyInfo.isInsideSecureHardware
    }

    private fun securityLevel(keyInfo: KeyInfo): String {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            keyInfo.securityLevel.toString()
        } else {
            "api_unavailable"
        }
    }

    private fun privateKeyEntry(alias: String): KeyStore.PrivateKeyEntry? {
        return openKeyStore().getEntry(alias, null) as? KeyStore.PrivateKeyEntry
    }

    private fun openKeyStore(): KeyStore {
        return KeyStore.getInstance(RadishLexAndroidKeystoreBridgeContract.PROVIDER)
            .apply { load(null) }
    }

    private fun deleteAlias(alias: String): String {
        return try {
            val keyStore = openKeyStore()
            keyStore.deleteEntry(alias)
            if (keyStore.containsAlias(alias)) "still_present" else "deleted"
        } catch (error: Exception) {
            "error:${error.javaClass.simpleName}"
        }
    }

    private fun encodedHead(encodedBytes: ByteArray?): String {
        return encodedBytes
            ?.take(20)
            ?.joinToString(separator = "") { "%02x".format(it) }
            ?: VALUE_UNAVAILABLE
    }

    private fun emit(message: String) {
        val line = "radishlex.android_keystore.diagnostics $message"
        println(line)
        Log.i(LOG_TAG, line)
    }

    companion object {
        private const val ARG_RUN_DIAGNOSTICS = "radishlex.runAndroidKeystoreDiagnostics"
        private const val LOG_TAG = "RadishLexKeystoreDiag"
        private const val VALUE_UNAVAILABLE = "unavailable"
        private val DIAGNOSTIC_CANONICAL_BYTES = byteArrayOf(
            0x72, 0x61, 0x64, 0x69, 0x73, 0x68, 0x6c, 0x65,
            0x78, 0x2d, 0x64, 0x69, 0x61, 0x67, 0x6e, 0x6f,
            0x73, 0x74, 0x69, 0x63, 0x2d, 0x76, 0x31
        )
    }
}

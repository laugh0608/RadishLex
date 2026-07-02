package org.radishlex.android.keystore

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.security.KeyFactory
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class RadishLexAndroidKeystoreBridgeInstrumentedTest {
    @Test
    fun createLoadSignAndDeleteSyntheticSigningKey() {
        val arguments = InstrumentationRegistry.getArguments()
        assumeTrue(
            "Set $ARG_RUN_SMOKE=true only after approving Android Keystore smoke on this device.",
            arguments.getString(ARG_RUN_SMOKE) == "true"
        )

        val signingKeyId = "radishlex-instrumented-smoke"
        val alias = "org.radishlex.sync.signing.instrumented.$signingKeyId"
        val bridge = RadishLexAndroidKeystoreBridge()
        val deleteRequest = RadishLexAndroidKeystoreBridgeRequest(
            contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
            operation = RadishLexAndroidKeystoreOperation.DeleteSigningKey.wireName,
            signingKeyId = signingKeyId,
            alias = alias
        )

        bridge.deleteSigningKey(deleteRequest)
        try {
            val publicKey = createSigningKey(bridge, signingKeyId, alias)
            val loadedPublicKey = loadPublicKey(bridge, signingKeyId, alias)
            assertArrayEquals(publicKey, loadedPublicKey)

            val canonicalBytes = byteArrayOf(
                0x72, 0x61, 0x64, 0x69, 0x73, 0x68, 0x6c, 0x65,
                0x78, 0x2d, 0x73, 0x69, 0x67, 0x6e, 0x61, 0x74,
                0x75, 0x72, 0x65, 0x2d, 0x76, 0x31
            )
            val signature = signCanonicalBytes(bridge, signingKeyId, alias, canonicalBytes)
            assertTrue(verifySignature(publicKey, canonicalBytes, signature))
        } finally {
            bridge.deleteSigningKey(deleteRequest)
        }

        val deleted = bridge.loadPublicKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.LoadPublicKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
        assertEquals(
            RadishLexAndroidKeystoreBridgeContract.ERROR_PRIVATE_KEY_UNAVAILABLE,
            deleted.errorCode
        )
    }

    private fun createSigningKey(
        bridge: RadishLexAndroidKeystoreBridge,
        signingKeyId: String,
        alias: String
    ): ByteArray {
        val result = bridge.createSigningKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.CreateSigningKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
        if (!result.isSuccess) {
            fail("Android Keystore create_signing_key returned ${result.errorCode}")
        }
        assertEquals(
            RadishLexAndroidKeystoreBridgeContract.RAW_ED25519_PUBLIC_KEY_SIZE,
            result.publicKey.size
        )
        return result.publicKey
    }

    private fun loadPublicKey(
        bridge: RadishLexAndroidKeystoreBridge,
        signingKeyId: String,
        alias: String
    ): ByteArray {
        val result = bridge.loadPublicKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.LoadPublicKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
        if (!result.isSuccess) {
            fail("Android Keystore load_public_key returned ${result.errorCode}")
        }
        assertEquals(
            RadishLexAndroidKeystoreBridgeContract.RAW_ED25519_PUBLIC_KEY_SIZE,
            result.publicKey.size
        )
        return result.publicKey
    }

    private fun signCanonicalBytes(
        bridge: RadishLexAndroidKeystoreBridge,
        signingKeyId: String,
        alias: String,
        canonicalBytes: ByteArray
    ): ByteArray {
        val result = bridge.sign(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = RadishLexAndroidKeystoreBridgeContract.CONTRACT_VERSION,
                operation = RadishLexAndroidKeystoreOperation.Sign.wireName,
                signingKeyId = signingKeyId,
                alias = alias,
                canonicalBytes = canonicalBytes
            )
        )
        if (!result.isSuccess) {
            fail("Android Keystore sign returned ${result.errorCode}")
        }
        assertEquals(
            RadishLexAndroidKeystoreBridgeContract.ED25519_SIGNATURE_SIZE,
            result.signature.size
        )
        return result.signature
    }

    private fun verifySignature(
        rawPublicKey: ByteArray,
        canonicalBytes: ByteArray,
        signatureBytes: ByteArray
    ): Boolean {
        val spkiPublicKey = ED25519_SPKI_PREFIX + rawPublicKey
        val publicKey = KeyFactory.getInstance(
            RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM
        ).generatePublic(X509EncodedKeySpec(spkiPublicKey))
        val verifier = Signature.getInstance(RadishLexAndroidKeystoreBridgeContract.SIGNATURE_ALGORITHM)
        verifier.initVerify(publicKey)
        verifier.update(canonicalBytes)
        return verifier.verify(signatureBytes)
    }

    companion object {
        private const val ARG_RUN_SMOKE = "radishlex.runAndroidKeystoreSmoke"

        private val ED25519_SPKI_PREFIX = byteArrayOf(
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65,
            0x70, 0x03, 0x21, 0x00
        )
    }
}

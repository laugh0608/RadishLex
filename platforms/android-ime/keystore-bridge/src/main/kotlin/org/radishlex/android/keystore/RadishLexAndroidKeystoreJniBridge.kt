package org.radishlex.android.keystore

object RadishLexAndroidKeystoreJniBridge {
    private val bridge = RadishLexAndroidKeystoreBridge()

    @JvmStatic
    fun createSigningKey(
        contractVersion: Int,
        signingKeyId: String,
        alias: String
    ): RadishLexAndroidKeystoreBridgeResult {
        return bridge.createSigningKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = contractVersion,
                operation = RadishLexAndroidKeystoreOperation.CreateSigningKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
    }

    @JvmStatic
    fun loadPublicKey(
        contractVersion: Int,
        signingKeyId: String,
        alias: String
    ): RadishLexAndroidKeystoreBridgeResult {
        return bridge.loadPublicKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = contractVersion,
                operation = RadishLexAndroidKeystoreOperation.LoadPublicKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
    }

    @JvmStatic
    fun sign(
        contractVersion: Int,
        signingKeyId: String,
        alias: String,
        canonicalBytes: ByteArray
    ): RadishLexAndroidKeystoreBridgeResult {
        return bridge.sign(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = contractVersion,
                operation = RadishLexAndroidKeystoreOperation.Sign.wireName,
                signingKeyId = signingKeyId,
                alias = alias,
                canonicalBytes = canonicalBytes
            )
        )
    }

    @JvmStatic
    fun deleteSigningKey(
        contractVersion: Int,
        signingKeyId: String,
        alias: String
    ): RadishLexAndroidKeystoreBridgeResult {
        return bridge.deleteSigningKey(
            RadishLexAndroidKeystoreBridgeRequest(
                contractVersion = contractVersion,
                operation = RadishLexAndroidKeystoreOperation.DeleteSigningKey.wireName,
                signingKeyId = signingKeyId,
                alias = alias
            )
        )
    }
}

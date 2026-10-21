package com.example.hellodoctor_mobile

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import org.json.JSONObject
import java.math.BigInteger
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.util.UUID
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.RejectedExecutionException

/** Installation private key is never exported or placed in Flutter storage. */
object DeviceProof {
    private const val ALIAS = "hellodoctor.session.proof.v1"
    private val worker = ThreadPoolExecutor(1, 1, 0L, TimeUnit.MILLISECONDS,
        ArrayBlockingQueue<Runnable>(8), ThreadPoolExecutor.AbortPolicy())
    private fun b64(bytes: ByteArray) = Base64.encodeToString(bytes, Base64.URL_SAFE or Base64.NO_WRAP or Base64.NO_PADDING)
    private fun coordinate(value: BigInteger): ByteArray {
        val raw = value.toByteArray()
        return ByteArray(32).also { out ->
            val size = minOf(32, raw.size)
            System.arraycopy(raw, raw.size - size, out, 32 - size, size)
        }
    }
    private fun store(): KeyStore {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (!store.containsAlias(ALIAS)) {
            val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
            generator.initialize(KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_SIGN)
                .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                .setDigests(KeyProperties.DIGEST_SHA256).build())
            generator.generateKeyPair()
        }
        return store
    }
    // Android produces ASN.1 ECDSA; JOSE requires fixed-width unsigned R || S.
    private fun joseSignature(der: ByteArray): ByteArray {
        require(der.size in 8..72 && der[0] == 0x30.toByte())
        var pos = 2
        require(der[pos++] == 0x02.toByte())
        val rSize = der[pos++].toInt() and 0xff
        require(rSize in 1..33 && pos + rSize < der.size)
        val r = coordinate(BigInteger(1, der.copyOfRange(pos, pos + rSize)))
        pos += rSize
        require(der[pos++] == 0x02.toByte())
        val sSize = der[pos++].toInt() and 0xff
        require(sSize in 1..33 && pos + sSize == der.size)
        val s = coordinate(BigInteger(1, der.copyOfRange(pos, pos + sSize)))
        return r + s
    }
    fun register(engine: FlutterEngine, activity: android.app.Activity) {
        MethodChannel(engine.dartExecutor.binaryMessenger, "hellodoctor/device_proof")
            .setMethodCallHandler { call, result ->
                if (call.method != "sign") { result.notImplemented(); return@setMethodCallHandler }
                val method = call.argument<String>("method")
                val path = call.argument<String>("path")
                val binding = call.argument<String>("binding")
                if (method == null || path == null || binding == null || !path.startsWith("/")) {
                    result.error("PROOF_INVALID", "Session proof unavailable", null)
                    return@setMethodCallHandler
                }
                try { worker.execute {
                    try {
                        val keyStore = store()
                        val publicKey = keyStore.getCertificate(ALIAS).publicKey as ECPublicKey
                        val jwk = JSONObject().put("kty", "EC").put("crv", "P-256")
                            .put("x", b64(coordinate(publicKey.w.affineX)))
                            .put("y", b64(coordinate(publicKey.w.affineY)))
                        val header = JSONObject().put("alg", "ES256").put("typ", "hd-device+jwt").put("jwk", jwk)
                        val body = JSONObject().put("htm", method).put("htu", path)
                            .put("iat", System.currentTimeMillis() / 1000).put("jti", UUID.randomUUID().toString())
                            .put("binding", b64(MessageDigest.getInstance("SHA-256").digest(binding.toByteArray(Charsets.UTF_8))))
                        val unsigned = b64(header.toString().toByteArray(Charsets.UTF_8)) + "." +
                            b64(body.toString().toByteArray(Charsets.UTF_8))
                        val signer = Signature.getInstance("SHA256withECDSA")
                        signer.initSign(keyStore.getKey(ALIAS, null) as java.security.PrivateKey)
                        signer.update(unsigned.toByteArray(Charsets.US_ASCII))
                        val proof = unsigned + "." + b64(joseSignature(signer.sign()))
                        activity.runOnUiThread { result.success(proof) }
                    } catch (_: Exception) {
                        activity.runOnUiThread { result.error("PROOF_UNAVAILABLE", "Please sign in again", null) }
                    }
                } } catch (_: RejectedExecutionException) {
                    result.error("PROOF_BUSY", "Please retry shortly", null)
                }
            }
    }
}

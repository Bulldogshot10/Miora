package __PACKAGE__

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Holds the SQLCipher database key encrypted by a non-exportable Android
 * Keystore AES key. The wrapped key file is deliberately kept under
 * noBackupFilesDir so cloud backup/device transfer cannot separate it from
 * the hardware/OS Keystore key that protects it.
 */
internal fun mioraDatabaseFile(context: Context): File =
    File(context.applicationContext.dataDir, "miora.sqlite3")

internal object MioraDatabaseKeyStore {
    private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    private const val WRAPPING_KEY_ALIAS = "com.bulldogshot10.miora.database-wrap-v1"
    private const val WIDGET_DRAFT_KEY_ALIAS = "com.bulldogshot10.miora.widget-draft-v1"
    private const val WRAPPED_KEY_FILE = "miora-database-key-v1.bin"
    private const val FORMAT_VERSION: Byte = 1
    private const val WIDGET_DRAFT_FORMAT_VERSION: Byte = 1
    private const val AES_GCM_IV_BYTES = 12
    private const val DATABASE_KEY_BYTES = 32
    private const val GCM_TAG_BITS = 128

    @Synchronized
    @Throws(Exception::class)
    fun encryptWidgetDraft(widgetId: Int, plaintext: String): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateAesKey(WIDGET_DRAFT_KEY_ALIAS))
        val iv = cipher.iv
        check(iv.size == AES_GCM_IV_BYTES) { "Unexpected Android Keystore GCM IV length." }
        cipher.updateAAD(widgetDraftAad(widgetId))

        val plaintextBytes = plaintext.toByteArray(Charsets.UTF_8)
        val ciphertext = try {
            cipher.doFinal(plaintextBytes)
        } finally {
            plaintextBytes.fill(0)
        }

        val encrypted = ByteArray(1 + iv.size + ciphertext.size)
        encrypted[0] = WIDGET_DRAFT_FORMAT_VERSION
        System.arraycopy(iv, 0, encrypted, 1, iv.size)
        System.arraycopy(ciphertext, 0, encrypted, 1 + iv.size, ciphertext.size)
        return Base64.encodeToString(encrypted, Base64.NO_WRAP)
    }

    @Synchronized
    @Throws(Exception::class)
    fun decryptWidgetDraft(widgetId: Int, encryptedValue: String): String {
        val encrypted = Base64.decode(encryptedValue, Base64.NO_WRAP)
        val minimumLength = 1 + AES_GCM_IV_BYTES + (GCM_TAG_BITS / 8)
        if (encrypted.size < minimumLength || encrypted[0] != WIDGET_DRAFT_FORMAT_VERSION) {
            throw IOException("The encrypted widget draft is malformed.")
        }

        val iv = encrypted.copyOfRange(1, 1 + AES_GCM_IV_BYTES)
        val ciphertext = encrypted.copyOfRange(1 + AES_GCM_IV_BYTES, encrypted.size)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(
            Cipher.DECRYPT_MODE,
            getOrCreateAesKey(WIDGET_DRAFT_KEY_ALIAS),
            GCMParameterSpec(GCM_TAG_BITS, iv),
        )
        cipher.updateAAD(widgetDraftAad(widgetId))
        val plaintext = cipher.doFinal(ciphertext)
        return try {
            String(plaintext, Charsets.UTF_8)
        } finally {
            plaintext.fill(0)
        }
    }

    private fun widgetDraftAad(widgetId: Int): ByteArray =
        "com.bulldogshot10.miora.widget-draft.v1:$widgetId".toByteArray(Charsets.UTF_8)

    @Synchronized
    @Throws(Exception::class)
    fun getOrCreateDatabaseKey(context: Context): ByteArray {
        val appContext = context.applicationContext
        val keyFile = File(appContext.noBackupFilesDir, WRAPPED_KEY_FILE)

        if (keyFile.exists()) {
            return decryptExistingKey(keyFile, getOrCreateWrappingKey())
        }

        // A missing wrapped key is recoverable only for a fresh install or a
        // legacy plaintext SQLite database awaiting its first SQLCipher migration.
        // Never mint a replacement key for an existing encrypted database.
        val databaseFile = mioraDatabaseFile(appContext)
        if (databaseFile.exists() && !hasPlaintextSqliteHeader(databaseFile)) {
            throw IOException(
                "The encrypted database exists but its Android Keystore key file is missing. " +
                    "Refusing to create a replacement key.",
            )
        }

        val wrappingKey = getOrCreateWrappingKey()
        val databaseKey = ByteArray(DATABASE_KEY_BYTES)
        SecureRandom().nextBytes(databaseKey)

        try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, wrappingKey)
            val iv = cipher.iv
            check(iv.size == AES_GCM_IV_BYTES) { "Unexpected Android Keystore GCM IV length." }

            val ciphertext = cipher.doFinal(databaseKey)
            val wrapped = ByteArray(1 + iv.size + ciphertext.size)
            wrapped[0] = FORMAT_VERSION
            System.arraycopy(iv, 0, wrapped, 1, iv.size)
            System.arraycopy(ciphertext, 0, wrapped, 1 + iv.size, ciphertext.size)

            val temporaryFile = File(keyFile.parentFile, "$WRAPPED_KEY_FILE.tmp")
            if (temporaryFile.exists() && !temporaryFile.delete()) {
                throw IOException("Could not clear an incomplete wrapped-key file.")
            }

            FileOutputStream(temporaryFile).use { output ->
                output.write(wrapped)
                output.fd.sync()
            }

            try {
                Files.move(
                    temporaryFile.toPath(),
                    keyFile.toPath(),
                    StandardCopyOption.ATOMIC_MOVE,
                )
            } catch (error: Exception) {
                temporaryFile.delete()
                // Fail closed: never leave an unwrapped SQLCipher key behind.
                throw IOException("Could not atomically store the wrapped database key.", error)
            }

            return databaseKey
        } catch (error: Exception) {
            databaseKey.fill(0)
            throw error
        }
    }

    private fun hasPlaintextSqliteHeader(file: File): Boolean {
        if (!file.isFile || file.length() < 16L) return false
        val expected = byteArrayOf(
            0x53, 0x51, 0x4c, 0x69, 0x74, 0x65, 0x20, 0x66,
            0x6f, 0x72, 0x6d, 0x61, 0x74, 0x20, 0x33, 0x00,
        )
        return file.inputStream().use { input ->
            val header = ByteArray(expected.size)
            var offset = 0
            while (offset < header.size) {
                val read = input.read(header, offset, header.size - offset)
                if (read < 0) return@use false
                offset += read
            }
            header.contentEquals(expected)
        }
    }

    private fun getOrCreateWrappingKey(): SecretKey =
        getOrCreateAesKey(WRAPPING_KEY_ALIAS)

    private fun getOrCreateAesKey(alias: String): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        val existing = keyStore.getKey(alias, null) as? SecretKey
        if (existing != null) return existing

        val generator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES,
            KEYSTORE_PROVIDER,
        )
        generator.init(
            KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setKeySize(256)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true)
                .setUserAuthenticationRequired(false)
                .build(),
        )
        return generator.generateKey()
    }

    private fun decryptExistingKey(file: File, wrappingKey: SecretKey): ByteArray {
        val wrapped = file.readBytes()
        val expectedLength = 1 + AES_GCM_IV_BYTES + DATABASE_KEY_BYTES + (GCM_TAG_BITS / 8)
        if (wrapped.size != expectedLength || wrapped[0] != FORMAT_VERSION) {
            throw IOException("The wrapped database key is malformed; refusing to replace it.")
        }

        val iv = wrapped.copyOfRange(1, 1 + AES_GCM_IV_BYTES)
        val ciphertext = wrapped.copyOfRange(1 + AES_GCM_IV_BYTES, wrapped.size)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, wrappingKey, GCMParameterSpec(GCM_TAG_BITS, iv))
        val databaseKey = cipher.doFinal(ciphertext)

        if (databaseKey.size != DATABASE_KEY_BYTES) {
            databaseKey.fill(0)
            throw IOException("The unwrapped database key has an invalid length.")
        }

        return databaseKey
    }
}

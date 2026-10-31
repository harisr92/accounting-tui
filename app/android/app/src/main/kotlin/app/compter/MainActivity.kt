package app.compter

import android.content.ActivityNotFoundException
import android.content.Intent
import androidx.core.content.FileProvider
import com.google.android.play.core.review.ReviewManagerFactory
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // lib/src/review_prompt.dart: Play decides whether the review dialog shows, and never
        // says, so success means only that it was asked
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "app.compter/review")
            .setMethodCallHandler { call, result ->
                if (call.method != "requestReview") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val manager = ReviewManagerFactory.create(this)
                manager.requestReviewFlow().addOnCompleteListener { request ->
                    if (request.isSuccessful) {
                        manager.launchReviewFlow(this, request.result)
                            .addOnCompleteListener { result.success(null) }
                    } else {
                        result.error("unavailable", request.exception?.message, null)
                    }
                }
            }
        // lib/src/notice.dart's openPdf: a PDF Rust wrote under the app's files, handed to the
        // phone's PDF viewer through res/xml/viewer_paths.xml
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "compter/viewer")
            .setMethodCallHandler { call, result ->
                val path = call.arguments as? String
                if (call.method != "open" || path == null) {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val file = File(path)
                if (!file.exists()) {
                    result.error("missing", "The PDF is no longer there", null)
                    return@setMethodCallHandler
                }
                val uri = FileProvider.getUriForFile(this, "$packageName.viewer", file)
                val view = Intent(Intent.ACTION_VIEW)
                    .setDataAndType(uri, "application/pdf")
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                try {
                    startActivity(view)
                    result.success(null)
                } catch (e: ActivityNotFoundException) {
                    result.error("noViewer", "No app on this phone opens PDFs", null)
                }
            }
    }
}

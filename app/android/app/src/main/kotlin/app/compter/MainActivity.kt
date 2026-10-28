package app.compter

import com.google.android.play.core.review.ReviewManagerFactory
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

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
    }
}

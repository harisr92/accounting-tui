# shared_preferences_android 2.5 calls its Kotlin API over JNI (Pigeon JNI), looking methods up
# by their signature, which names kotlin.coroutines.Continuation. R8 renames that class unless
# it's kept, and every preference read then fails with NoSuchMethodError in release builds.
-keep interface kotlin.coroutines.Continuation

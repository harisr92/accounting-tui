# shared_preferences_android 2.5 calls its Kotlin API over JNI (Pigeon JNI). Its bindings, and
# the jni package that resumes their suspend calls, look classes, methods and fields up by name,
# which R8 can't see, so it strips or renames them and every preference read and write fails in
# release builds (ClassNotFoundException, NoSuchMethodError). The plugin's own classes:
-keep class io.flutter.plugins.sharedpreferences.** { *; }
# and every kotlin/ class the bindings of jni 1.1 and shared_preferences_android 2.5 name; check
# them again when either is upgraded:
#   grep -rhoE 'kotlin/[A-Za-z0-9_/$]+' ~/.pub-cache/hosted/pub.dev/{jni,shared_preferences_android}-*/lib | sort -u
-keep class kotlin.coroutines.** { *; }
-keep class kotlin.Result { *; }
-keep class kotlin.Result$Failure { *; }
-keep class kotlin.Unit { *; }
-keep class kotlin.enums.EnumEntries { *; }
-keep class kotlin.jvm.internal.DefaultConstructorMarker { *; }

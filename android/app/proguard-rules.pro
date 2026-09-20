-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**

# kotlinx.serialization 生成的 serializer
-keepclassmembers class com.simonlei.tinyreader.data.** {
    *** Companion;
}
-keepclasseswithmembers class com.simonlei.tinyreader.data.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# OkHttp
-dontwarn okhttp3.**
-dontwarn okio.**

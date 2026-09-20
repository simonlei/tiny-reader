package com.simonlei.tinyreader.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.sp

/** 取自桌面端 src/styles/main.css 的 CSS 变量 */
val Bg = Color(0xFF14161A)
val BgAlt = Color(0xFF101216)
val Panel = Color(0xFF1A1D23)
val Panel2 = Color(0xFF21252D)
val BorderColor = Color(0xFF2B3038)
val TextMain = Color(0xFFE6E8EB)
val TextDim = Color(0xFFA2ABB9)
val TextMute = Color(0xFF6F7887)
val Accent = Color(0xFF4C8DFF)
val AccentSoft = Color(0x244C8DFF)
val Danger = Color(0xFFEF5B5B)
val Warn = Color(0xFFE0A33A)
val Ok = Color(0xFF3FB950)

private val TinyReaderColors = darkColorScheme(
    primary = Accent,
    onPrimary = Color.White,
    primaryContainer = AccentSoft,
    onPrimaryContainer = TextMain,
    secondary = Accent,
    onSecondary = Color.White,
    secondaryContainer = AccentSoft,
    onSecondaryContainer = TextMain,
    tertiary = Warn,
    onTertiary = Color.Black,
    background = Bg,
    onBackground = TextMain,
    surface = Panel,
    onSurface = TextMain,
    surfaceVariant = Panel2,
    onSurfaceVariant = TextDim,
    surfaceContainer = Panel,
    surfaceContainerHigh = Panel2,
    surfaceContainerHighest = Panel2,
    outline = BorderColor,
    outlineVariant = BorderColor,
    error = Danger,
    onError = Color.White,
    errorContainer = Color(0xFF3A1D1D),
    onErrorContainer = Color(0xFFFFD9D9),
    scrim = Color(0xCC000000),
)

private val TinyReaderTypography = Typography(
    bodyLarge = TextStyle(fontSize = 15.sp, lineHeight = 22.sp),
    bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontSize = 12.sp, lineHeight = 17.sp),
    titleMedium = TextStyle(fontSize = 16.sp, lineHeight = 22.sp),
    labelLarge = TextStyle(fontSize = 14.sp, lineHeight = 18.sp),
    labelSmall = TextStyle(fontSize = 11.sp, lineHeight = 14.sp),
)

/** 应用只提供暗色主题，与桌面端观感保持一致 */
@Composable
fun TinyReaderTheme(
    @Suppress("UNUSED_PARAMETER") darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = TinyReaderColors,
        typography = TinyReaderTypography,
        content = content,
    )
}

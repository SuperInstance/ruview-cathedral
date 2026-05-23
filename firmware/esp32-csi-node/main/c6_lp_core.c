/**
 * @file c6_lp_core.c
 * @brief LP-core wake-on-motion hibernation — ADR-110 Phase 5 skeleton.
 *
 * The actual LP-core binary lives in a separate component subproject
 * compiled with the LP RISC-V toolchain (`riscv32-esp-elf` with LP-core
 * memory layout). For the P5 skeleton we ship just the HP-side arming
 * + deep-sleep entry, using esp_sleep_enable_ext1_wakeup() as the wake
 * source. A follow-up turn will replace ext1 with a true LP-core
 * polling program that can debounce / threshold the accelerometer
 * signal in software, dropping standby current from ~10 µA to ~5 µA.
 */

#include "sdkconfig.h"

#if defined(CONFIG_IDF_TARGET_ESP32C6) && defined(CONFIG_ULP_COPROC_TYPE_LP_CORE)

#include "c6_lp_core.h"
#include "esp_log.h"
#include "esp_sleep.h"
#include "driver/rtc_io.h"
#include "soc/soc_caps.h"

static const char *TAG = "c6_lp";

static int  s_wake_gpio   = -1;
static bool s_active_high = true;
static bool s_armed       = false;

esp_err_t c6_lp_core_arm(int wake_gpio, bool active_high)
{
    if (wake_gpio < 0) {
        ESP_LOGE(TAG, "invalid wake_gpio=%d", wake_gpio);
        return ESP_ERR_INVALID_ARG;
    }
    s_wake_gpio   = wake_gpio;
    s_active_high = active_high;

    /* GPIO must be in the LP/RTC domain for deep-sleep wake. */
    esp_err_t ret = rtc_gpio_init(wake_gpio);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "rtc_gpio_init(%d) failed: %s", wake_gpio, esp_err_to_name(ret));
        return ret;
    }
    rtc_gpio_set_direction(wake_gpio, RTC_GPIO_MODE_INPUT_ONLY);

    /* On the C6, deep-sleep GPIO wake is esp_deep_sleep_enable_gpio_wakeup. */
    uint64_t mask = 1ULL << wake_gpio;
    esp_deepsleep_gpio_wake_up_mode_t mode = active_high
        ? ESP_GPIO_WAKEUP_GPIO_HIGH
        : ESP_GPIO_WAKEUP_GPIO_LOW;
    ret = esp_deep_sleep_enable_gpio_wakeup(mask, mode);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "enable_gpio_wakeup failed: %s", esp_err_to_name(ret));
        return ret;
    }

    s_armed = true;
    ESP_LOGI(TAG, "armed: wake_gpio=%d active_%s",
             wake_gpio, active_high ? "high" : "low");
    return ESP_OK;
}

void c6_lp_core_hibernate_and_wait(void)
{
    if (!s_armed) {
        ESP_LOGW(TAG, "hibernate called without arm — sleeping with no wake source");
    }
    /* Configure for hibernation: power down everything except what's needed
     * to retain the wake source. On C6 the RTC peripheral domain is the
     * only one we need to gate explicitly — RTC_SLOW_MEM / RTC_FAST_MEM
     * aren't separate power domains on the C6 SoC. */
    esp_sleep_pd_config(ESP_PD_DOMAIN_RTC_PERIPH, ESP_PD_OPTION_OFF);

    ESP_LOGI(TAG, "entering deep sleep — target ≤5 µA");
    esp_deep_sleep_start();
    /* Never returns. */
}

bool c6_lp_core_was_motion_wake(void)
{
    esp_sleep_wakeup_cause_t cause = esp_sleep_get_wakeup_cause();
    return cause == ESP_SLEEP_WAKEUP_GPIO || cause == ESP_SLEEP_WAKEUP_EXT1;
}

#endif  /* CONFIG_IDF_TARGET_ESP32C6 && CONFIG_ULP_COPROC_TYPE_LP_CORE */

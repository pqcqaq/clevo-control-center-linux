// SPDX-License-Identifier: GPL-2.0
/*
 * Clevo/BlueSky Control Center ACPI bridge.
 *
 * This mirrors the Windows InsydeDCHU.dll call:
 *   \_SB.DCHU._DSM(UUID=93f224e4-fbdc-4bbf-add6-db71bdc0afad,
 *                  revision=1, function=0x67,
 *                  package(buffer[0x100] = { G, R, B, zone }))
 */

#include <linux/acpi.h>
#include <linux/ctype.h>
#include <linux/init.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/proc_fs.h>
#include <linux/slab.h>
#include <linux/string.h>
#include <linux/uaccess.h>

#define LED_PROC_NAME "clevo_control_center_led"
#define DCHU_CONTROL_PROC_NAME "clevo_dchu_control"
#define DCHU_CONFIG_PROC_NAME "clevo_dchu_config"
#define DCHU_STATUS_PROC_NAME "clevo_dchu_status"
#define DCHU_APP_SETTINGS_PROC_NAME "clevo_dchu_app_settings"
#define DCHU_PATH "\\_SB.DCHU"
#define DCHU_FUNCTION 0x67
#define DCHU_BUFFER_SIZE 0x100
#define DCHU_APP_SETTINGS_SIZE 0x1000
#define DCHU_APP_POWER_MODE_OFFSET ((1u << 8) + 1u)
#define DCHU_APP_FAN_MODE_OFFSET ((4u << 8) + 5u)
#define DCHU_MAX_OUTPUT (DCHU_BUFFER_SIZE * 3 + 128)
#define FAN_CURVE_POINTS 4
#define FAN_CURVE_MIN_TEMP 30
#define FAN_CURVE_MAX_TEMP 100
#define FAN_CURVE_MIN_DUTY 0
#define FAN_CURVE_MAX_DUTY 100

struct fan_curve_point {
	u8 temp;
	u8 duty;
};

static const guid_t dchu_guid =
	GUID_INIT(0x93f224e4, 0xfbdc, 0x4bbf,
		  0xad, 0xd6, 0xdb, 0x71, 0xbd, 0xc0, 0xaf, 0xad);

static struct proc_dir_entry *led_proc_entry;
static struct proc_dir_entry *dchu_control_proc_entry;
static struct proc_dir_entry *dchu_config_proc_entry;
static struct proc_dir_entry *dchu_status_proc_entry;
static struct proc_dir_entry *dchu_app_settings_proc_entry;
static acpi_handle dchu_handle;
static u8 dchu_app_settings[DCHU_APP_SETTINGS_SIZE];
static bool dchu_app_power_mode_valid;
static bool dchu_app_fan_mode_valid;
static DEFINE_MUTEX(dchu_app_settings_lock);
static bool verbose;

struct dchu_result {
	char text[DCHU_MAX_OUTPUT];
	size_t len;
};

module_param(verbose, bool, 0644);
MODULE_PARM_DESC(verbose, "Log every keyboard RGB update");

static int clevo_dchu_eval(u32 function, const u8 *payload, size_t payload_len,
			   struct dchu_result *result)
{
	union acpi_object argv4[4];
	union acpi_object package_element;
	struct acpi_object_list input;
	struct acpi_buffer output = { ACPI_ALLOCATE_BUFFER, NULL };
	u8 buffer[DCHU_BUFFER_SIZE] = { 0 };
	acpi_status status;
	int ret = 0;
	size_t offset = 0;

	if (!dchu_handle)
		return -ENODEV;

	if (payload_len > sizeof(buffer))
		return -EINVAL;
	if (payload && payload_len)
		memcpy(buffer, payload, payload_len);

	argv4[0].type = ACPI_TYPE_BUFFER;
	argv4[0].buffer.pointer = (u8 *)&dchu_guid;
	argv4[0].buffer.length = 16;

	argv4[1].type = ACPI_TYPE_INTEGER;
	argv4[1].integer.value = 1;

	argv4[2].type = ACPI_TYPE_INTEGER;
	argv4[2].integer.value = function;

	package_element.type = ACPI_TYPE_BUFFER;
	package_element.buffer.pointer = buffer;
	package_element.buffer.length = sizeof(buffer);

	argv4[3].type = ACPI_TYPE_PACKAGE;
	argv4[3].package.count = 1;
	argv4[3].package.elements = &package_element;

	input.count = ARRAY_SIZE(argv4);
	input.pointer = argv4;

	status = acpi_evaluate_object(dchu_handle, "_DSM", &input, &output);
	if (ACPI_FAILURE(status)) {
		pr_err("clevo_control_center: _DSM function=0x%02x failed: %s\n",
		       function, acpi_format_exception(status));
		return -EIO;
	}

	if (output.pointer) {
		union acpi_object *obj = output.pointer;

		if (result) {
			if (obj->type == ACPI_TYPE_INTEGER) {
				result->len = scnprintf(result->text, sizeof(result->text),
							"integer 0x%llx\n",
							obj->integer.value);
			} else if (obj->type == ACPI_TYPE_BUFFER) {
				offset += scnprintf(result->text + offset,
						    sizeof(result->text) - offset,
						    "buffer %u\n", obj->buffer.length);
				for (size_t i = 0; i < obj->buffer.length && offset < sizeof(result->text); i++) {
					offset += scnprintf(result->text + offset,
							    sizeof(result->text) - offset,
							    "%02x%s",
							    obj->buffer.pointer[i],
							    ((i + 1) % 16 == 0) ? "\n" : " ");
				}
				if (offset < sizeof(result->text) && offset > 0 &&
				    result->text[offset - 1] != '\n')
					offset += scnprintf(result->text + offset,
							    sizeof(result->text) - offset, "\n");
				result->len = offset;
			} else {
				result->len = scnprintf(result->text, sizeof(result->text),
							"type %u\n", obj->type);
			}
		} else if (obj->type == ACPI_TYPE_INTEGER && obj->integer.value != function) {
			pr_warn("clevo_control_center: unexpected _DSM function=0x%02x return 0x%llx\n",
				function, obj->integer.value);
			ret = -EIO;
		}
		ACPI_FREE(output.pointer);
	} else if (result) {
		result->len = scnprintf(result->text, sizeof(result->text), "null\n");
	}

	return ret;
}

static int clevo_dchu_eval_buffer(u32 function, const u8 *payload, size_t payload_len,
				  u8 *result_buffer, size_t result_buffer_len,
				  size_t *result_len)
{
	union acpi_object argv4[4];
	union acpi_object package_element;
	struct acpi_object_list input;
	struct acpi_buffer output = { ACPI_ALLOCATE_BUFFER, NULL };
	u8 buffer[DCHU_BUFFER_SIZE] = { 0 };
	acpi_status status;
	int ret = 0;

	if (!dchu_handle)
		return -ENODEV;
	if (!result_buffer || !result_len)
		return -EINVAL;
	if (payload_len > sizeof(buffer))
		return -EINVAL;
	if (payload && payload_len)
		memcpy(buffer, payload, payload_len);

	argv4[0].type = ACPI_TYPE_BUFFER;
	argv4[0].buffer.pointer = (u8 *)&dchu_guid;
	argv4[0].buffer.length = 16;

	argv4[1].type = ACPI_TYPE_INTEGER;
	argv4[1].integer.value = 1;

	argv4[2].type = ACPI_TYPE_INTEGER;
	argv4[2].integer.value = function;

	package_element.type = ACPI_TYPE_BUFFER;
	package_element.buffer.pointer = buffer;
	package_element.buffer.length = sizeof(buffer);

	argv4[3].type = ACPI_TYPE_PACKAGE;
	argv4[3].package.count = 1;
	argv4[3].package.elements = &package_element;

	input.count = ARRAY_SIZE(argv4);
	input.pointer = argv4;

	status = acpi_evaluate_object(dchu_handle, "_DSM", &input, &output);
	if (ACPI_FAILURE(status)) {
		pr_err("clevo_control_center: _DSM function=0x%02x failed: %s\n",
		       function, acpi_format_exception(status));
		return -EIO;
	}

	if (output.pointer) {
		union acpi_object *obj = output.pointer;

		if (obj->type == ACPI_TYPE_BUFFER) {
			*result_len = min_t(size_t, obj->buffer.length, result_buffer_len);
			memcpy(result_buffer, obj->buffer.pointer, *result_len);
		} else {
			ret = -EIO;
		}
		ACPI_FREE(output.pointer);
	} else {
		ret = -EIO;
	}

	return ret;
}

static size_t clevo_dchu_append_gpu_mux_info(char *output, size_t output_size)
{
	u8 payload[DCHU_BUFFER_SIZE] = { 0 };
	u8 result[DCHU_BUFFER_SIZE] = { 0 };
	size_t result_len = 0;
	size_t offset = 0;
	u32 version;
	int ret;

	payload[0] = 8;
	ret = clevo_dchu_eval_buffer(0x04, payload, sizeof(payload),
				     result, sizeof(result), &result_len);
	if (!ret && result_len > 18) {
		version = ((u32)result[0] << 8) | result[1];
		offset += scnprintf(output + offset, output_size - offset,
				    "bios_feature_04_08_version integer 0x%x\n",
				    version);
		offset += scnprintf(output + offset, output_size - offset,
				    "bios_feature_04_08_offset18 integer 0x%02x\n",
				    result[18]);
	} else {
		offset += scnprintf(output + offset, output_size - offset,
				    "bios_feature_04_08_version unknown\n");
		offset += scnprintf(output + offset, output_size - offset,
				    "bios_feature_04_08_offset18 unknown\n");
	}

	memset(payload, 0, sizeof(payload));
	memset(result, 0, sizeof(result));
	result_len = 0;
	payload[0] = 21;
	ret = clevo_dchu_eval_buffer(0x04, payload, sizeof(payload),
				     result, sizeof(result), &result_len);
	if (!ret && result_len > 1) {
		offset += scnprintf(output + offset, output_size - offset,
				    "gpu_mux_04_15_current integer 0x%02x\n",
				    result[0]);
		offset += scnprintf(output + offset, output_size - offset,
				    "gpu_mux_04_15_options integer 0x%02x\n",
				    result[1]);
	} else {
		offset += scnprintf(output + offset, output_size - offset,
				    "gpu_mux_04_15_current unknown\n");
		offset += scnprintf(output + offset, output_size - offset,
				    "gpu_mux_04_15_options unknown\n");
	}

	return offset;
}

static int clevo_dchu_set_zone_rgb(u8 zone, u8 r, u8 g, u8 b)
{
	u8 payload[4] = { g, r, b, zone };
	int ret = clevo_dchu_eval(DCHU_FUNCTION, payload, sizeof(payload), NULL);

	if (!ret && verbose)
		pr_info("clevo_control_center: set zone=0x%02x rgb=%02x%02x%02x\n",
			zone, r, g, b);
	return ret;
}

static bool clevo_led_zone_allowed(unsigned int zone)
{
	return zone >= 0xf0 && zone <= 0xf6;
}

static int parse_hex_byte(const char *s, u8 *out)
{
	unsigned int value;

	if (!isxdigit(s[0]) || !isxdigit(s[1]))
		return -EINVAL;
	if (sscanf(s, "%2x", &value) != 1 || value > 0xff)
		return -EINVAL;

	*out = value;
	return 0;
}

static int parse_rgb_hex(const char *s, u8 *r, u8 *g, u8 *b)
{
	if (strlen(s) != 6)
		return -EINVAL;

	if (parse_hex_byte(s, r) ||
	    parse_hex_byte(s + 2, g) ||
	    parse_hex_byte(s + 4, b))
		return -EINVAL;

	return 0;
}

static ssize_t clevo_led_write(struct file *file, const char __user *ubuf,
			       size_t count, loff_t *ppos)
{
	char buf[64];
	char first[16] = { 0 };
	char second[16] = { 0 };
	char extra[2] = { 0 };
	char *p;
	unsigned int zone_int = 0xff;
	u8 r, g, b, zone;
	int matched;
	int ret;

	if (count == 0)
		return 0;
	if (count >= sizeof(buf))
		return -EINVAL;

	if (copy_from_user(buf, ubuf, count))
		return -EFAULT;
	buf[count] = '\0';

	p = strim(buf);

	matched = sscanf(p, "%15s %15s %1s", first, second, extra);
	if (matched == 1) {
		if (parse_rgb_hex(first, &r, &g, &b))
			return -EINVAL;
	} else if (matched == 2) {
		if (kstrtouint(first, 16, &zone_int))
			return -EINVAL;
		if (parse_rgb_hex(second, &r, &g, &b))
			return -EINVAL;
	} else {
		return -EINVAL;
	}

	if (zone_int == 0xff) {
		ret = clevo_dchu_set_zone_rgb(0xf0, r, g, b);
		if (!ret)
			ret = clevo_dchu_set_zone_rgb(0xf1, r, g, b);
		if (!ret)
			ret = clevo_dchu_set_zone_rgb(0xf2, r, g, b);
	} else {
		if (zone_int > 0xff)
			return -EINVAL;
		if (!clevo_led_zone_allowed(zone_int))
			return -EINVAL;
		zone = (u8)zone_int;
		ret = clevo_dchu_set_zone_rgb(zone, r, g, b);
	}

	return ret ? ret : count;
}

static ssize_t clevo_led_read(struct file *file, char __user *ubuf,
			      size_t count, loff_t *ppos)
{
	const char *help =
		"Usage:\n"
		"  echo ff0000 > /proc/clevo_control_center_led       # all 3 zones red\n"
		"  echo 'f0 00ff00' > /proc/clevo_control_center_led  # zone 0xf0 green\n"
		"Zones: explicit zone writes are limited to f0..f6.\n";

	return simple_read_from_buffer(ubuf, count, ppos, help, strlen(help));
}

static const struct proc_ops clevo_led_proc_ops = {
	.proc_read = clevo_led_read,
	.proc_write = clevo_led_write,
};

static int parse_fan_mode_name(const char *value, u32 *mode)
{
	if (!strcmp(value, "auto")) {
		*mode = 0;
	} else if (!strcmp(value, "max")) {
		*mode = 1;
	} else if (!strcmp(value, "silent")) {
		*mode = 3;
	} else if (!strcmp(value, "maxq")) {
		*mode = 5;
	} else if (!strcmp(value, "custom")) {
		*mode = 6;
	} else {
		return kstrtou32(value, 10, mode);
	}

	return 0;
}

static bool dchu_fan_mode_allowed(u32 mode)
{
	switch (mode) {
	case 0:
	case 1:
	case 3:
	case 5:
	case 6:
		return true;
	default:
		return false;
	}
}

static void clevo_dchu_app_write_byte(u32 page, u32 offset, u8 value)
{
	u32 app_offset = (page << 8) + offset;

	if (app_offset >= sizeof(dchu_app_settings))
		return;

	mutex_lock(&dchu_app_settings_lock);
	dchu_app_settings[app_offset] = value;
	if (app_offset == DCHU_APP_POWER_MODE_OFFSET)
		dchu_app_power_mode_valid = true;
	else if (app_offset == DCHU_APP_FAN_MODE_OFFSET)
		dchu_app_fan_mode_valid = true;
	mutex_unlock(&dchu_app_settings_lock);
}

static int clevo_dchu_set_fan_mode(const char *value)
{
	u32 mode;
	u32 payload;
	int ret;

	ret = parse_fan_mode_name(value, &mode);
	if (ret)
		return ret;

	if (!dchu_fan_mode_allowed(mode))
		return -EINVAL;

	payload = (0x01u << 24) | mode;
	ret = clevo_dchu_eval(0x79, (u8 *)&payload, sizeof(payload), NULL);
	if (!ret)
		clevo_dchu_app_write_byte(4, 5, (u8)mode);
	return ret;
}

static int clevo_dchu_set_power_mode(const char *value)
{
	u32 mode;
	u32 payload;
	u8 old_mode;
	bool old_valid;
	int ret;

	ret = kstrtou32(value, 10, &mode);
	if (ret)
		return ret;
	if (mode > 3)
		return -EINVAL;

	mutex_lock(&dchu_app_settings_lock);
	old_mode = dchu_app_settings[DCHU_APP_POWER_MODE_OFFSET];
	old_valid = dchu_app_power_mode_valid;
	mutex_unlock(&dchu_app_settings_lock);

	clevo_dchu_app_write_byte(1, 1, (u8)mode);
	payload = (0x19u << 24) | mode;
	ret = clevo_dchu_eval(0x79, (u8 *)&payload, sizeof(payload), NULL);
	if (ret) {
		mutex_lock(&dchu_app_settings_lock);
		dchu_app_settings[DCHU_APP_POWER_MODE_OFFSET] = old_mode;
		dchu_app_power_mode_valid = old_valid;
		mutex_unlock(&dchu_app_settings_lock);
	}
	return ret;
}

static u8 fan_curve_duty_raw(u8 duty)
{
	return DIV_ROUND_CLOSEST((u32)duty * 255u, 100u);
}

static u16 fan_curve_slope(const struct fan_curve_point *from,
			   const struct fan_curve_point *to)
{
	u32 duty_delta = to->duty - from->duty;
	u32 temp_delta = to->temp - from->temp;

	return DIV_ROUND_CLOSEST(duty_delta * 255u * 16u, 100u * temp_delta);
}

static int parse_fan_curve_points(char *value, struct fan_curve_point *points)
{
	char *cursor = value;
	char *token;
	unsigned int temp;
	unsigned int duty;
	char extra;
	int index = 0;

	while ((token = strsep(&cursor, ",")) != NULL) {
		if (!*token || index >= FAN_CURVE_POINTS)
			return -EINVAL;
		if (sscanf(token, "%3u:%3u%c", &temp, &duty, &extra) != 2)
			return -EINVAL;
		if (temp < FAN_CURVE_MIN_TEMP || temp > FAN_CURVE_MAX_TEMP)
			return -EINVAL;
		if (duty > FAN_CURVE_MAX_DUTY)
			return -EINVAL;
		if (index > 0) {
			if (temp <= points[index - 1].temp)
				return -EINVAL;
			if (duty < points[index - 1].duty)
				return -EINVAL;
		}
		points[index].temp = (u8)temp;
		points[index].duty = (u8)duty;
		index++;
	}

	return index == FAN_CURVE_POINTS ? 0 : -EINVAL;
}

static void fan_curve_payload_channel(u8 *payload, int channel,
				      const struct fan_curve_point *points)
{
	int point_base = 2 + channel * 4;
	int slope_base = 14 + channel * 6;
	u16 slope;

	payload[point_base] = points[1].temp;
	payload[point_base + 1] = fan_curve_duty_raw(points[1].duty);
	payload[point_base + 2] = points[2].temp;
	payload[point_base + 3] = fan_curve_duty_raw(points[2].duty);

	slope = fan_curve_slope(&points[0], &points[1]);
	payload[slope_base] = (u8)(slope >> 8);
	payload[slope_base + 1] = (u8)slope;
	slope = fan_curve_slope(&points[1], &points[2]);
	payload[slope_base + 2] = (u8)(slope >> 8);
	payload[slope_base + 3] = (u8)slope;
	slope = fan_curve_slope(&points[2], &points[3]);
	payload[slope_base + 4] = (u8)(slope >> 8);
	payload[slope_base + 5] = (u8)slope;
}

static int clevo_dchu_set_fan_curve(char *cpu_value, char *gpu_value)
{
	struct fan_curve_point cpu[FAN_CURVE_POINTS];
	struct fan_curve_point gpu[FAN_CURVE_POINTS];
	u8 payload[DCHU_BUFFER_SIZE] = { 0 };
	int ret;

	ret = parse_fan_curve_points(cpu_value, cpu);
	if (ret)
		return ret;
	ret = parse_fan_curve_points(gpu_value, gpu);
	if (ret)
		return ret;

	fan_curve_payload_channel(payload, 0, cpu);
	fan_curve_payload_channel(payload, 1, gpu);
	fan_curve_payload_channel(payload, 2, gpu);

	ret = clevo_dchu_eval(0x0e, payload, sizeof(payload), NULL);
	if (ret)
		return ret;

	return clevo_dchu_set_fan_mode("custom");
}

static ssize_t clevo_dchu_control_write(struct file *file, const char __user *ubuf,
					size_t count, loff_t *ppos)
{
	char buf[160];
	char command[24] = { 0 };
	char value[48] = { 0 };
	char value2[48] = { 0 };
	char extra[2] = { 0 };
	char *p;
	int matched;
	int ret;

	if (count == 0)
		return 0;
	if (count >= sizeof(buf))
		return -EINVAL;
	if (copy_from_user(buf, ubuf, count))
		return -EFAULT;
	buf[count] = '\0';
	p = strim(buf);

	matched = sscanf(p, "%23s %47s %47s %1s", command, value, value2, extra);

	if (!strcmp(command, "fan-mode")) {
		if (matched != 2)
			return -EINVAL;
		ret = clevo_dchu_set_fan_mode(value);
	} else if (!strcmp(command, "power-mode")) {
		if (matched != 2)
			return -EINVAL;
		ret = clevo_dchu_set_power_mode(value);
	} else if (!strcmp(command, "fan-curve")) {
		if (matched != 3)
			return -EINVAL;
		ret = clevo_dchu_set_fan_curve(value, value2);
	} else {
		return -EINVAL;
	}

	return ret ? ret : count;
}

static ssize_t clevo_dchu_control_read(struct file *file, char __user *ubuf,
				       size_t count, loff_t *ppos)
{
	const char *help =
		"Usage:\n"
		"  echo 'fan-mode auto' > /proc/clevo_dchu_control\n"
		"  echo 'fan-mode max' > /proc/clevo_dchu_control\n"
		"  echo 'fan-mode silent' > /proc/clevo_dchu_control\n"
		"  echo 'power-mode 2' > /proc/clevo_dchu_control\n"
		"  echo 'fan-curve 40:28,58:42,78:72,100:100 42:25,60:44,80:74,100:100' > /proc/clevo_dchu_control\n";

	return simple_read_from_buffer(ubuf, count, ppos, help, strlen(help));
}

static const struct proc_ops clevo_dchu_control_proc_ops = {
	.proc_read = clevo_dchu_control_read,
	.proc_write = clevo_dchu_control_write,
};

static size_t clevo_dchu_app_settings_format(char *output, size_t output_size)
{
	size_t offset = 0;
	bool power_valid;
	bool fan_valid;
	u8 power_mode;
	u8 fan_mode;

	mutex_lock(&dchu_app_settings_lock);
	power_valid = dchu_app_power_mode_valid;
	fan_valid = dchu_app_fan_mode_valid;
	power_mode = dchu_app_settings[DCHU_APP_POWER_MODE_OFFSET];
	fan_mode = dchu_app_settings[DCHU_APP_FAN_MODE_OFFSET];
	mutex_unlock(&dchu_app_settings_lock);

	offset += scnprintf(output + offset, output_size - offset,
			    "app_power_mode ");
	if (power_valid)
		offset += scnprintf(output + offset, output_size - offset,
				    "%u\n", power_mode);
	else
		offset += scnprintf(output + offset, output_size - offset,
				    "unknown\n");

	offset += scnprintf(output + offset, output_size - offset,
			    "app_fan_mode ");
	if (fan_valid)
		offset += scnprintf(output + offset, output_size - offset,
				    "%u\n", fan_mode);
	else
		offset += scnprintf(output + offset, output_size - offset,
				    "unknown\n");

	return offset;
}

static ssize_t clevo_dchu_app_settings_read(struct file *file, char __user *ubuf,
					    size_t count, loff_t *ppos)
{
	char output[96];
	size_t len = clevo_dchu_app_settings_format(output, sizeof(output));

	return simple_read_from_buffer(ubuf, count, ppos, output, len);
}

static const struct proc_ops clevo_dchu_app_settings_proc_ops = {
	.proc_read = clevo_dchu_app_settings_read,
};

static ssize_t clevo_dchu_config_read(struct file *file, char __user *ubuf,
				      size_t count, loff_t *ppos)
{
	struct dchu_result *result;
	char *output;
	size_t output_size = DCHU_MAX_OUTPUT + 384;
	size_t offset = 0;
	int ret;

	output = kzalloc(output_size, GFP_KERNEL);
	if (!output)
		return -ENOMEM;

	result = kzalloc(sizeof(*result), GFP_KERNEL);
	if (!result) {
		kfree(output);
		return -ENOMEM;
	}

	ret = clevo_dchu_eval(0x0d, NULL, 0, result);
	if (ret)
		goto out;

	offset += scnprintf(output + offset, output_size - offset, "config_0d ");
	offset += scnprintf(output + offset, output_size - offset, "%s", result->text);
	ret = clevo_dchu_eval(0x10, NULL, 0, result);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf5_10 %s", result->text);
	ret = clevo_dchu_eval(0x52, NULL, 0, result);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf1_52 %s", result->text);
	ret = clevo_dchu_eval(0x60, NULL, 0, result);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf4_60 %s", result->text);
	ret = clevo_dchu_eval(0x7a, NULL, 0, result);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf2_7a %s", result->text);
	offset += clevo_dchu_append_gpu_mux_info(output + offset, output_size - offset);
	offset += clevo_dchu_app_settings_format(output + offset, output_size - offset);

	ret = simple_read_from_buffer(ubuf, count, ppos, output, offset);

out:
	kfree(result);
	kfree(output);
	return ret;
}

static const struct proc_ops clevo_dchu_config_proc_ops = {
	.proc_read = clevo_dchu_config_read,
};

static ssize_t clevo_dchu_status_read(struct file *file, char __user *ubuf,
				      size_t count, loff_t *ppos)
{
	struct dchu_result result = { 0 };
	int ret;

	ret = clevo_dchu_eval(0x0c, NULL, 0, &result);
	if (ret)
		return ret;

	return simple_read_from_buffer(ubuf, count, ppos, result.text, result.len);
}

static const struct proc_ops clevo_dchu_status_proc_ops = {
	.proc_read = clevo_dchu_status_read,
};

static int __init clevo_control_center_init(void)
{
	acpi_status status;

	status = acpi_get_handle(NULL, DCHU_PATH, &dchu_handle);
	if (ACPI_FAILURE(status)) {
		pr_err("clevo_control_center: cannot get ACPI handle %s: %s\n",
		       DCHU_PATH, acpi_format_exception(status));
		return -ENODEV;
	}

	led_proc_entry = proc_create(LED_PROC_NAME, 0666, NULL, &clevo_led_proc_ops);
	if (!led_proc_entry)
		return -ENOMEM;

	dchu_control_proc_entry = proc_create(DCHU_CONTROL_PROC_NAME, 0666, NULL,
					      &clevo_dchu_control_proc_ops);
	if (!dchu_control_proc_entry) {
		proc_remove(led_proc_entry);
		return -ENOMEM;
	}

	dchu_status_proc_entry = proc_create(DCHU_STATUS_PROC_NAME, 0444, NULL,
					     &clevo_dchu_status_proc_ops);
	if (!dchu_status_proc_entry) {
		proc_remove(dchu_control_proc_entry);
		proc_remove(led_proc_entry);
		return -ENOMEM;
	}

	dchu_config_proc_entry = proc_create(DCHU_CONFIG_PROC_NAME, 0444, NULL,
					     &clevo_dchu_config_proc_ops);
	if (!dchu_config_proc_entry) {
		proc_remove(dchu_status_proc_entry);
		proc_remove(dchu_control_proc_entry);
		proc_remove(led_proc_entry);
		return -ENOMEM;
	}

	dchu_app_settings_proc_entry = proc_create(DCHU_APP_SETTINGS_PROC_NAME, 0444, NULL,
						   &clevo_dchu_app_settings_proc_ops);
	if (!dchu_app_settings_proc_entry) {
		proc_remove(dchu_config_proc_entry);
		proc_remove(dchu_status_proc_entry);
		proc_remove(dchu_control_proc_entry);
		proc_remove(led_proc_entry);
		return -ENOMEM;
	}

	pr_info("clevo_control_center: loaded, LED at /proc/%s; DCHU status at /proc/%s; DCHU config at /proc/%s; DCHU control at /proc/%s; DCHU app settings at /proc/%s\n",
		LED_PROC_NAME, DCHU_STATUS_PROC_NAME, DCHU_CONFIG_PROC_NAME,
		DCHU_CONTROL_PROC_NAME, DCHU_APP_SETTINGS_PROC_NAME);
	return 0;
}

static void __exit clevo_control_center_exit(void)
{
	proc_remove(dchu_app_settings_proc_entry);
	proc_remove(dchu_config_proc_entry);
	proc_remove(dchu_status_proc_entry);
	proc_remove(dchu_control_proc_entry);
	proc_remove(led_proc_entry);
	pr_info("clevo_control_center: unloaded\n");
}

module_init(clevo_control_center_init);
module_exit(clevo_control_center_exit);

MODULE_LICENSE("GPL");
MODULE_AUTHOR("Codex");
MODULE_DESCRIPTION("Clevo/BlueSky control center ACPI bridge");

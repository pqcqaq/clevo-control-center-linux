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
#include <linux/proc_fs.h>
#include <linux/slab.h>
#include <linux/uaccess.h>

#define LED_PROC_NAME "clevo_control_center_led"
#define DCHU_CONTROL_PROC_NAME "clevo_dchu_control"
#define DCHU_CONFIG_PROC_NAME "clevo_dchu_config"
#define DCHU_STATUS_PROC_NAME "clevo_dchu_status"
#define DCHU_PATH "\\_SB.DCHU"
#define DCHU_FUNCTION 0x67
#define DCHU_BUFFER_SIZE 0x100
#define DCHU_MAX_OUTPUT (DCHU_BUFFER_SIZE * 3 + 128)

static const guid_t dchu_guid =
	GUID_INIT(0x93f224e4, 0xfbdc, 0x4bbf,
		  0xad, 0xd6, 0xdb, 0x71, 0xbd, 0xc0, 0xaf, 0xad);

static struct proc_dir_entry *led_proc_entry;
static struct proc_dir_entry *dchu_control_proc_entry;
static struct proc_dir_entry *dchu_config_proc_entry;
static struct proc_dir_entry *dchu_status_proc_entry;
static acpi_handle dchu_handle;
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
		*mode = 2;
	} else if (!strcmp(value, "maxq")) {
		*mode = 5;
	} else if (!strcmp(value, "custom")) {
		*mode = 6;
	} else {
		return kstrtou32(value, 10, mode);
	}

	return 0;
}

static int clevo_dchu_set_fan_mode(const char *value)
{
	u32 mode;
	u32 payload;
	int ret;

	ret = parse_fan_mode_name(value, &mode);
	if (ret)
		return ret;

	switch (mode) {
	case 0:
	case 1:
	case 2:
	case 5:
	case 6:
		break;
	default:
		return -EINVAL;
	}

	payload = (0x01u << 24) | mode;
	return clevo_dchu_eval(0x79, (u8 *)&payload, sizeof(payload), NULL);
}

static int clevo_dchu_set_power_mode(const char *value)
{
	u32 mode;
	u32 payload;
	int ret;

	ret = kstrtou32(value, 10, &mode);
	if (ret)
		return ret;
	if (mode > 3)
		return -EINVAL;

	payload = (0x19u << 24) | mode;
	return clevo_dchu_eval(0x79, (u8 *)&payload, sizeof(payload), NULL);
}

static ssize_t clevo_dchu_control_write(struct file *file, const char __user *ubuf,
					size_t count, loff_t *ppos)
{
	char buf[96];
	char command[24] = { 0 };
	char value[24] = { 0 };
	char extra[2] = { 0 };
	char *p;
	int ret;

	if (count == 0)
		return 0;
	if (count >= sizeof(buf))
		return -EINVAL;
	if (copy_from_user(buf, ubuf, count))
		return -EFAULT;
	buf[count] = '\0';
	p = strim(buf);

	if (sscanf(p, "%23s %23s %1s", command, value, extra) != 2)
		return -EINVAL;

	if (!strcmp(command, "fan-mode"))
		ret = clevo_dchu_set_fan_mode(value);
	else if (!strcmp(command, "power-mode"))
		ret = clevo_dchu_set_power_mode(value);
	else
		return -EINVAL;

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
		"  echo 'power-mode 2' > /proc/clevo_dchu_control\n";

	return simple_read_from_buffer(ubuf, count, ppos, help, strlen(help));
}

static const struct proc_ops clevo_dchu_control_proc_ops = {
	.proc_read = clevo_dchu_control_read,
	.proc_write = clevo_dchu_control_write,
};

static ssize_t clevo_dchu_config_read(struct file *file, char __user *ubuf,
				      size_t count, loff_t *ppos)
{
	struct dchu_result config = { 0 };
	struct dchu_result feature = { 0 };
	char *output;
	size_t output_size = DCHU_MAX_OUTPUT + 256;
	size_t offset = 0;
	int ret;

	output = kzalloc(output_size, GFP_KERNEL);
	if (!output)
		return -ENOMEM;

	ret = clevo_dchu_eval(0x0d, NULL, 0, &config);
	if (ret)
		goto out;

	offset += scnprintf(output + offset, output_size - offset, "config_0d ");
	offset += scnprintf(output + offset, output_size - offset, "%s", config.text);
	ret = clevo_dchu_eval(0x10, NULL, 0, &feature);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf5_10 %s", feature.text);
	ret = clevo_dchu_eval(0x52, NULL, 0, &feature);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf1_52 %s", feature.text);
	ret = clevo_dchu_eval(0x60, NULL, 0, &feature);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf4_60 %s", feature.text);
	ret = clevo_dchu_eval(0x7a, NULL, 0, &feature);
	if (ret)
		goto out;
	offset += scnprintf(output + offset, output_size - offset, "psf2_7a %s", feature.text);

	ret = simple_read_from_buffer(ubuf, count, ppos, output, offset);

out:
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

	pr_info("clevo_control_center: loaded, LED at /proc/%s; DCHU status at /proc/%s; DCHU config at /proc/%s; DCHU control at /proc/%s\n",
		LED_PROC_NAME, DCHU_STATUS_PROC_NAME, DCHU_CONFIG_PROC_NAME,
		DCHU_CONTROL_PROC_NAME);
	return 0;
}

static void __exit clevo_control_center_exit(void)
{
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

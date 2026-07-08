// SPDX-License-Identifier: GPL-2.0
/*
 * Minimal test driver for Clevo/Insyde DCHU keyboard RGB control.
 *
 * This mirrors the Windows InsydeDCHU.dll call:
 *   \_SB.DCHU._DSM(UUID=93f224e4-fbdc-4bbf-add6-db71bdc0afad,
 *                  revision=1, function=0x67,
 *                  package(buffer[0x100] = { G, R, B, zone }))
 */

#include <linux/acpi.h>
#include <linux/init.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/proc_fs.h>
#include <linux/uaccess.h>

#define LED_PROC_NAME "clevo_kbd_led"
#define DCHU_PROC_NAME "clevo_dchu"
#define DCHU_PATH "\\_SB.DCHU"
#define DCHU_FUNCTION 0x67
#define DCHU_BUFFER_SIZE 0x100
#define DCHU_MAX_OUTPUT (DCHU_BUFFER_SIZE * 3 + 128)

static const guid_t dchu_guid =
	GUID_INIT(0x93f224e4, 0xfbdc, 0x4bbf,
		  0xad, 0xd6, 0xdb, 0x71, 0xbd, 0xc0, 0xaf, 0xad);

static struct proc_dir_entry *led_proc_entry;
static struct proc_dir_entry *dchu_proc_entry;
static acpi_handle dchu_handle;
static bool verbose;

struct dchu_result {
	char text[DCHU_MAX_OUTPUT];
	size_t len;
};

static struct dchu_result last_dchu_result;

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
		pr_err("clevo_kbd_led: _DSM function=0x%02x failed: %s\n",
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
			pr_warn("clevo_kbd_led: unexpected _DSM function=0x%02x return 0x%llx\n",
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
		pr_info("clevo_kbd_led: set zone=0x%02x rgb=%02x%02x%02x\n",
			zone, r, g, b);
	return ret;
}

static int parse_hex_byte(const char *s, u8 *out)
{
	unsigned int value;

	if (sscanf(s, "%2x", &value) != 1 || value > 0xff)
		return -EINVAL;

	*out = value;
	return 0;
}

static ssize_t clevo_kbd_led_write(struct file *file, const char __user *ubuf,
				   size_t count, loff_t *ppos)
{
	char buf[64];
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

	matched = sscanf(p, "%x %2hhx%2hhx%2hhx", &zone_int, &r, &g, &b);
	if (matched != 4) {
		zone_int = 0xff;
		if (strlen(p) < 6 ||
		    parse_hex_byte(p, &r) ||
		    parse_hex_byte(p + 2, &g) ||
		    parse_hex_byte(p + 4, &b))
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
		zone = (u8)zone_int;
		ret = clevo_dchu_set_zone_rgb(zone, r, g, b);
	}

	return ret ? ret : count;
}

static ssize_t clevo_kbd_led_read(struct file *file, char __user *ubuf,
				  size_t count, loff_t *ppos)
{
	const char *help =
		"Usage:\n"
		"  echo ff0000 | sudo tee /proc/clevo_kbd_led       # all 3 zones red\n"
		"  echo 'f0 00ff00' | sudo tee /proc/clevo_kbd_led  # zone 0xf0 green\n"
		"Zones: f0, f1, f2 are the three Windows app zones.\n";

	return simple_read_from_buffer(ubuf, count, ppos, help, strlen(help));
}

static const struct proc_ops clevo_kbd_led_proc_ops = {
	.proc_read = clevo_kbd_led_read,
	.proc_write = clevo_kbd_led_write,
};

static int parse_hex_payload(char *text, u8 *payload, size_t *payload_len)
{
	char *token;
	size_t len = 0;
	unsigned int value;

	while ((token = strsep(&text, " \t\r\n")) != NULL) {
		if (!*token)
			continue;
		if (len >= DCHU_BUFFER_SIZE)
			return -EINVAL;
		if (strlen(token) > 2 || sscanf(token, "%2x", &value) != 1 || value > 0xff)
			return -EINVAL;
		payload[len++] = value;
	}

	*payload_len = len;
	return 0;
}

static ssize_t clevo_dchu_write(struct file *file, const char __user *ubuf,
				size_t count, loff_t *ppos)
{
	char buf[768];
	char *p;
	char op[16] = { 0 };
	unsigned int function;
	u8 payload[DCHU_BUFFER_SIZE] = { 0 };
	size_t payload_len = 0;
	int ret;

	if (count == 0)
		return 0;
	if (count >= sizeof(buf))
		return -EINVAL;
	if (copy_from_user(buf, ubuf, count))
		return -EFAULT;
	buf[count] = '\0';
	p = strim(buf);

	if (sscanf(p, "%15s %x", op, &function) < 2)
		return -EINVAL;
	if (function > 0xff)
		return -EINVAL;

	p = strchr(p, ' ');
	if (!p)
		return -EINVAL;
	p = skip_spaces(p);
	p = strchr(p, ' ');
	if (p) {
		p = skip_spaces(p);
		ret = parse_hex_payload(p, payload, &payload_len);
		if (ret)
			return ret;
	}

	if (!strcmp(op, "read")) {
		ret = clevo_dchu_eval(function, NULL, 0, &last_dchu_result);
	} else if (!strcmp(op, "write")) {
		ret = clevo_dchu_eval(function, payload, payload_len, &last_dchu_result);
	} else {
		return -EINVAL;
	}

	return ret ? ret : count;
}

static ssize_t clevo_dchu_read(struct file *file, char __user *ubuf,
			       size_t count, loff_t *ppos)
{
	const char *help =
		"Usage:\n"
		"  echo 'read 0c' > /proc/clevo_dchu\n"
		"  echo 'read 0d' > /proc/clevo_dchu\n"
		"  echo 'write 0e <payload bytes>' > /proc/clevo_dchu\n"
		"  cat /proc/clevo_dchu\n";

	if (last_dchu_result.len == 0)
		return simple_read_from_buffer(ubuf, count, ppos, help, strlen(help));

	return simple_read_from_buffer(ubuf, count, ppos,
				       last_dchu_result.text, last_dchu_result.len);
}

static const struct proc_ops clevo_dchu_proc_ops = {
	.proc_read = clevo_dchu_read,
	.proc_write = clevo_dchu_write,
};

static int __init clevo_kbd_led_init(void)
{
	acpi_status status;

	status = acpi_get_handle(NULL, DCHU_PATH, &dchu_handle);
	if (ACPI_FAILURE(status)) {
		pr_err("clevo_kbd_led: cannot get ACPI handle %s: %s\n",
		       DCHU_PATH, acpi_format_exception(status));
		return -ENODEV;
	}

	led_proc_entry = proc_create(LED_PROC_NAME, 0666, NULL, &clevo_kbd_led_proc_ops);
	if (!led_proc_entry)
		return -ENOMEM;

	dchu_proc_entry = proc_create(DCHU_PROC_NAME, 0600, NULL, &clevo_dchu_proc_ops);
	if (!dchu_proc_entry) {
		proc_remove(led_proc_entry);
		return -ENOMEM;
	}

	pr_info("clevo_kbd_led: loaded, write RGB hex to /proc/%s; DCHU debug at /proc/%s\n",
		LED_PROC_NAME, DCHU_PROC_NAME);
	return 0;
}

static void __exit clevo_kbd_led_exit(void)
{
	proc_remove(dchu_proc_entry);
	proc_remove(led_proc_entry);
	pr_info("clevo_kbd_led: unloaded\n");
}

module_init(clevo_kbd_led_init);
module_exit(clevo_kbd_led_exit);

MODULE_LICENSE("GPL");
MODULE_AUTHOR("Codex");
MODULE_DESCRIPTION("Minimal Clevo/Insyde DCHU keyboard RGB ACPI test driver");

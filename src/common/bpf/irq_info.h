#ifndef IRQ_INFO_H
#define IRQ_INFO_H

#define IRQ_NAME_LEN 64

struct rezolus_irq_info {
	int id;
	u8 name[IRQ_NAME_LEN];
};

#endif //IRQ_INFO_H